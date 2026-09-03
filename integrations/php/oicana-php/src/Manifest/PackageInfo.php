<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * The Typst package a template is.
 */
final readonly class PackageInfo
{
    /**
     * @param string $name Name of the template
     * @param string $version Version of the template
     * @param string $entrypoint File the compilation starts at
     * @param list<string> $authors Authors of the template
     * @param string|null $license License of the template
     * @param string|null $description Short description of the template
     * @param string|null $homepage Web presence of the template
     * @param string|null $repository Repository the template is developed in
     */
    public function __construct(
        public string $name,
        public string $version,
        public string $entrypoint,
        public array $authors,
        public ?string $license,
        public ?string $description,
        public ?string $homepage,
        public ?string $repository
    ) {
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        return new self(
            $data['name'],
            $data['version'],
            $data['entrypoint'],
            $data['authors'],
            $data['license'],
            $data['description'],
            $data['homepage'],
            $data['repository']
        );
    }
}
