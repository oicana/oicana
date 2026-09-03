<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * A template's manifest.
 */
final readonly class TemplateManifest
{
    /**
     * @param PackageInfo $package The Typst package section of the manifest
     * @param OicanaConfig $oicana The Oicana section of the manifest
     */
    public function __construct(
        public PackageInfo $package,
        public OicanaConfig $oicana
    ) {
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        return new self(
            PackageInfo::fromArray($data['package']),
            OicanaConfig::fromArray($data['oicana'])
        );
    }
}
