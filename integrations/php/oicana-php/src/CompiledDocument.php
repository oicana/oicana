<?php

declare(strict_types=1);

namespace Oicana;

/**
 * A compiled document kept in memory so it can be exported on demand without
 * re-compiling.
 *
 * Obtain one via {@see Template::compile()}. Call {@see close()} when done to
 * free the underlying document (the destructor does so as a fallback).
 *
 * Example:
 * ```php
 * $document = $template->compile(jsonInputs: ['name' => '{"value": "Alice"}']);
 * try {
 *     $pdf = $document->exportPdf();
 *     $firstPagePng = $document->exportPng(pixelsPerPt: 2.0, pages: PageRange::single(0));
 * } finally {
 *     $document->close();
 * }
 * ```
 */
final class CompiledDocument
{
    /**
     * Sizes (in points) of every page, in document order.
     *
     * @var list<PageSize>
     */
    public readonly array $pages;

    /**
     * Warnings produced by the compilation of this document, or null if there
     * were none.
     */
    public readonly ?string $warnings;

    private ?string $documentId;

    /**
     * @internal Use {@see Template::compile()}.
     *
     * @throws \JsonException If the page metadata cannot be decoded
     */
    public function __construct(string $documentId)
    {
        $this->documentId = $documentId;

        $pagesJson = \OicanaInternal\document_pages($documentId);
        /** @var list<array{width: float, height: float}> $sizes */
        $sizes = json_decode($pagesJson, true, flags: JSON_THROW_ON_ERROR);
        $this->pages = array_map(
            static fn(array $size): PageSize => new PageSize(
                (float) $size['width'],
                (float) $size['height']
            ),
            $sizes
        );

        /** @var string|null $warnings */
        $warnings = \OicanaInternal\get_warnings($documentId);
        $this->warnings = $warnings;
    }

    /**
     * Number of pages in the document.
     */
    public function pageCount(): int
    {
        return count($this->pages);
    }

    /**
     * Export the document in the given format (defaults to PDF), optionally
     * restricted to a range of pages.
     *
     * @param ExportFormat|null $exportFormat Export format configuration (defaults to PDF)
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string Document bytes (PDF, PNG, or SVG)
     * @throws OicanaException If export fails
     */
    public function export(?ExportFormat $exportFormat = null, ?PageRange $pages = null): string
    {
        $this->ensureOpen();

        $formatArray = ($exportFormat ?? ExportFormat::pdf())->toArray();
        return \OicanaInternal\export_document(
            $this->documentId,
            json_encode($formatArray, JSON_THROW_ON_ERROR),
            $pages?->toNative()
        );
    }

    /**
     * Export the document to PDF, optionally restricted to a range of pages.
     * Tagging will be automatically turned off when exporting a subset of pages.
     *
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string PDF bytes
     * @throws OicanaException If export fails
     */
    public function exportPdf(?PageRange $pages = null): string
    {
        return $this->export(ExportFormat::pdf(), $pages);
    }

    /**
     * Export the document to PNG, optionally restricted to a range of pages.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param float $pixelsPerPt Resolution in pixels per point (defaults to 1.0)
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string PNG bytes
     * @throws OicanaException If export fails
     */
    public function exportPng(float $pixelsPerPt = 1.0, ?PageRange $pages = null): string
    {
        return $this->export(ExportFormat::png($pixelsPerPt), $pages);
    }

    /**
     * Export the document to SVG, optionally restricted to a range of pages.
     *
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string SVG bytes
     * @throws OicanaException If export fails
     */
    public function exportSvg(?PageRange $pages = null): string
    {
        return $this->export(ExportFormat::svg(), $pages);
    }

    /**
     * Release the cached document. The instance must not be used afterward.
     */
    public function close(): void
    {
        if ($this->documentId !== null) {
            \OicanaInternal\remove_document($this->documentId);
            $this->documentId = null;
        }
    }

    /**
     * Destructor ensures the document is freed even if close() was not called.
     */
    public function __destruct()
    {
        try {
            $this->close();
        } catch (\Throwable $e) {
            // Best effort cleanup - don't throw in destructor
        }
    }

    private function ensureOpen(): void
    {
        if ($this->documentId === null) {
            throw new OicanaException('CompiledDocument has already been closed');
        }
    }
}
