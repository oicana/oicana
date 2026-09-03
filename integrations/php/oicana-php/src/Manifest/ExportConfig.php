<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * How compiled documents are exported.
 */
final readonly class ExportConfig
{
    public function __construct(
        public PdfExportConfig $pdf
    ) {
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        return new self(new PdfExportConfig($data['pdf']['standards'], $data['pdf']['tagged']));
    }
}
