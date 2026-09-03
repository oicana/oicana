<?php

declare(strict_types=1);

namespace Oicana;

use Oicana\Inputs\BlobInput;
use Oicana\Manifest\TemplateManifest;

/**
 * Oicana template for PDF generation.
 *
 * This class provides an idiomatic PHP interface to the Oicana native extension.
 * Templates are based on Typst and can generate PDFs, PNGs, and SVGs.
 *
 * Example:
 * ```php
 * $template = new Template($templateBytes);
 * try {
 *     $pdf = $template->export(
 *         jsonInputs: ['name' => '{"value": "Alice"}'],
 *         exportFormat: ExportFormat::pdf()
 *     );
 *     file_put_contents('output.pdf', $pdf);
 * } finally {
 *     $template->cleanup();
 * }
 * ```
 */
class Template
{
    private string $templateId;
    private ?string $lastWarnings = null;

    /**
     * Initialize template and register it with the native extension.
     *
     * @param string $template Template zip file bytes
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs blob inputs
     * @param CompilationMode $mode Compilation mode
     * @param ZipLimits|null $limits Limits for reading the template zip (defaults apply when null)
     * @throws \RuntimeException If the oicana extension is not loaded
     * @throws \Exception If template registration fails
     */
    public function __construct(
        string $template,
        array $jsonInputs = [],
        array $blobInputs = [],
        CompilationMode $mode = CompilationMode::Development,
        ?ZipLimits $limits = null
    ) {
        if (!extension_loaded('oicana')) {
            throw new \RuntimeException(
                'The oicana PHP extension is not loaded. '
                . 'Run "vendor/bin/oicana-env" to get the activation command for your platform, '
                . 'or add the extension to your php.ini. '
                . 'See https://oicana.com/docs for installation instructions.'
            );
        }

        $this->templateId = uniqid('template_', true);

        $jsonInputs = self::encodeJsonInputs($jsonInputs);
        $nativeBlobs = $this->prepareBlobInputs($blobInputs);

        $docId = \OicanaInternal\register_template(
            $this->templateId,
            $template,
            $jsonInputs,
            $nativeBlobs,
            $mode->toNative(),
            $limits?->maxEntries,
            $limits?->maxTotalDecompressedBytes
        );

        $this->lastWarnings = \OicanaInternal\get_warnings($docId);
        \OicanaInternal\remove_document($docId);
    }

    /**
     * Warnings produced by the most recent compilation, or null if there were none.
     */
    public function warnings(): ?string
    {
        return $this->lastWarnings;
    }

    /**
     * Compile template and export to the given format.
     *
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param ExportFormat|null $exportFormat Export format configuration (defaults to PDF)
     * @param CompilationMode $mode Compilation mode
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string Compiled document bytes (PDF, PNG, or SVG)
     * @throws \Exception If compilation or export fails
     */
    public function export(
        array $jsonInputs = [],
        array $blobInputs = [],
        ?ExportFormat $exportFormat = null,
        CompilationMode $mode = CompilationMode::Production,
        ?PageRange $pages = null
    ): string {
        $formatArray = ($exportFormat ?? ExportFormat::pdf())->toArray();

        $jsonInputs = self::encodeJsonInputs($jsonInputs);
        $nativeBlobs = $this->prepareBlobInputs($blobInputs);

        $docId = \OicanaInternal\compile_template(
            $this->templateId,
            $jsonInputs,
            $nativeBlobs,
            $mode->toNative()
        );

        $this->lastWarnings = \OicanaInternal\get_warnings($docId);

        try {
            return \OicanaInternal\export_document(
                $docId,
                json_encode($formatArray),
                $pages?->toNative()
            );
        } finally {
            \OicanaInternal\remove_document($docId);
        }
    }

    /**
     * Compile template and export to PDF in a single call.
     * Tagging will be automatically turned off when exporting a subset of pages.
     *
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param CompilationMode $mode Compilation mode
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string PDF bytes
     * @throws \Exception If compilation or export fails
     */
    public function exportPdf(
        array $jsonInputs = [],
        array $blobInputs = [],
        CompilationMode $mode = CompilationMode::Production,
        ?PageRange $pages = null
    ): string {
        return $this->export($jsonInputs, $blobInputs, ExportFormat::pdf(), $mode, $pages);
    }

    /**
     * Compile template and export to PNG in a single call.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param CompilationMode $mode Compilation mode
     * @param float $pixelsPerPt Resolution in pixels per point (defaults to 1.0)
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string PNG bytes
     * @throws \Exception If compilation or export fails
     */
    public function exportPng(
        array $jsonInputs = [],
        array $blobInputs = [],
        CompilationMode $mode = CompilationMode::Production,
        float $pixelsPerPt = 1.0,
        ?PageRange $pages = null
    ): string {
        return $this->export($jsonInputs, $blobInputs, ExportFormat::png($pixelsPerPt), $mode, $pages);
    }

    /**
     * Compile template and export to SVG in a single call.
     *
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param CompilationMode $mode Compilation mode
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @return string SVG bytes
     * @throws \Exception If compilation or export fails
     */
    public function exportSvg(
        array $jsonInputs = [],
        array $blobInputs = [],
        CompilationMode $mode = CompilationMode::Production,
        ?PageRange $pages = null
    ): string {
        return $this->export($jsonInputs, $blobInputs, ExportFormat::svg(), $mode, $pages);
    }

    /**
     * Compile the template.
     *
     * Unlike {@see export()}, the document is kept in memory so it can be
     * exported one or more times without re-compiling. Call
     * {@see CompiledDocument::close()} when done to free it.
     *
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param CompilationMode $mode Compilation mode
     * @return CompiledDocument A handle to the compiled document
     * @throws \Exception If compilation fails
     */
    public function compile(
        array $jsonInputs = [],
        array $blobInputs = [],
        CompilationMode $mode = CompilationMode::Production
    ): CompiledDocument {
        $jsonInputs = self::encodeJsonInputs($jsonInputs);
        $nativeBlobs = $this->prepareBlobInputs($blobInputs);

        $docId = \OicanaInternal\compile_template(
            $this->templateId,
            $jsonInputs,
            $nativeBlobs,
            $mode->toNative()
        );

        $document = new CompiledDocument($docId);
        $this->lastWarnings = $document->warnings;
        return $document;
    }

    /**
     * Compile and export the given template once without caching.
     *
     * This is a convenience method for one-off compilations where you don't need
     * to reuse the template. For multiple compilations with the same template,
     * create an instance of Template and use export() instead.
     *
     * @param string $templateBytes Template zip file bytes
     * @param array<string, string|array<mixed>> $jsonInputs JSON inputs (key => JSON string or array)
     * @param array<string, BlobInput> $blobInputs Blob inputs
     * @param ExportFormat|null $exportFormat Export format configuration (defaults to PDF)
     * @param CompilationMode $mode Compilation mode
     * @param PageRange|null $pages 0-based, inclusive page range (defaults to the whole document)
     * @param ZipLimits|null $limits Limits for reading the template zip (defaults apply when null)
     * @return ExportOnceResult The exported document and any compilation warnings
     * @throws \Exception If compilation or export fails
     */
    public static function exportOnce(
        string $templateBytes,
        array $jsonInputs = [],
        array $blobInputs = [],
        ?ExportFormat $exportFormat = null,
        CompilationMode $mode = CompilationMode::Production,
        ?PageRange $pages = null,
        ?ZipLimits $limits = null
    ): ExportOnceResult {
        $formatArray = ($exportFormat ?? ExportFormat::pdf())->toArray();

        $jsonInputs = self::encodeJsonInputs($jsonInputs);
        $nativeBlobs = self::prepareBlobInputsStatic($blobInputs);

        $result = \OicanaInternal\export_template_once(
            $templateBytes,
            $jsonInputs,
            $nativeBlobs,
            $mode->toNative(),
            json_encode($formatArray),
            $pages?->toNative(),
            $limits?->maxEntries,
            $limits?->maxTotalDecompressedBytes
        );

        return new ExportOnceResult($result->document(), $result->warnings);
    }

    /**
     * Get the template's manifest.
     *
     * Returns the Typst package section and the Oicana configuration of the
     * template, including its input definitions.
     *
     * @return TemplateManifest The template manifest
     * @throws \Exception If template is not registered
     */
    public function manifest(): TemplateManifest
    {
        $manifestJson = \OicanaInternal\manifest($this->templateId);
        return TemplateManifest::fromArray(json_decode($manifestJson, true));
    }

    /**
     * Get source file content from template.
     *
     * @param string $path File path within the template
     * @return string Source code as string
     * @throws \Exception If file not found or template not registered
     */
    public function source(string $path): string
    {
        return \OicanaInternal\get_source($this->templateId, $path);
    }

    /**
     * Get binary file content from template.
     *
     * @param string $path File path within the template
     * @return string Binary file content
     * @throws \Exception If file not found or template not registered
     */
    public function file(string $path): string
    {
        return \OicanaInternal\get_file($this->templateId, $path);
    }

    /**
     * Clean up cached resources.
     *
     * This should be called when you're done with the template to free memory.
     * It's automatically called by the destructor, but explicit cleanup is recommended.
     */
    public function cleanup(): void
    {
        \OicanaInternal\remove_world($this->templateId);
    }

    /**
     * Enable or disable JSON schema validation for this template.
     *
     * When enabled (the default), JSON inputs are validated against their schemas
     * before compilation.
     *
     * @param bool $validate Whether to validate inputs against their JSON schemas.
     */
    public function setValidateInputs(bool $validate): void
    {
        \OicanaInternal\set_validate_inputs($this->templateId, $validate);
    }

    /**
     * Configure automatic cache eviction after each compilation.
     *
     * @param int|null $maxAge Maximum age threshold, or null to disable:
     *   - null - Disables cache eviction (cache never cleared)
     *   - 0 - Clears all cache entries with every eviction
     *   - 1 - Keeps only entries used since the last eviction
     *   - n - Keeps entries used within the last n evictions
     *   Default is 10.
     */
    public static function configureAutomaticCacheEviction(?int $maxAge): void
    {
        \OicanaInternal\configure_automatic_cache_eviction($maxAge);
    }

    /**
     * Manually evict the cache with the given age threshold.
     *
     * This directly calls the underlying eviction with the specified age,
     * regardless of the configured default age.
     *
     * @param int $maxAge Maximum age threshold for eviction.
     *   Entries with age >= this value will be removed.
     */
    public static function evictCache(int $maxAge): void
    {
        \OicanaInternal\evict_cache($maxAge);
    }

    /**
     * Destructor ensures cleanup even if not called explicitly.
     */
    public function __destruct()
    {
        try {
            $this->cleanup();
        } catch (\Throwable $e) {
            // Best effort cleanup - don't throw in destructor
        }
    }

    /**
     * Encode array values in jsonInputs to JSON strings.
     *
     * @param array<string, string|array<mixed>> $jsonInputs
     * @return array<string, string>
     */
    private static function encodeJsonInputs(array $jsonInputs): array
    {
        $encoded = [];
        foreach ($jsonInputs as $key => $value) {
            $encoded[$key] = is_array($value)
                ? json_encode($value, JSON_THROW_ON_ERROR)
                : $value;
        }
        return $encoded;
    }

    /**
     * Convert BlobInput objects to native BlobWithMetadata objects.
     *
     * @param array<string, BlobInput> $blobInputs
     * @return array<string, \OicanaInternal\BlobWithMetadata>
     */
    private function prepareBlobInputs(array $blobInputs): array
    {
        return self::prepareBlobInputsStatic($blobInputs);
    }

    /**
     * Convert BlobInput objects to native BlobWithMetadata objects.
     *
     * @param array<string, BlobInput> $blobInputs
     * @return array<string, \OicanaInternal\BlobWithMetadata>
     */
    private static function prepareBlobInputsStatic(array $blobInputs): array
    {
        $nativeBlobs = [];
        foreach ($blobInputs as $key => $blob) {
            $meta = json_encode((object) ($blob->metadata ?? []), JSON_THROW_ON_ERROR);
            $nativeBlobs[$key] = new \OicanaInternal\BlobWithMetadata($blob->data, $meta);
        }
        return $nativeBlobs;
    }
}
