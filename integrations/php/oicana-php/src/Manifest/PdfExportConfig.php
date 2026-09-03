<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * How documents are exported to PDF.
 */
final readonly class PdfExportConfig
{
    /**
     * @param list<string> $standards PDF standards the export conforms to, for example `a-3b`
     * @param bool $tagged Whether the PDF is tagged for accessibility
     */
    public function __construct(
        public array $standards,
        public bool $tagged
    ) {
    }
}
