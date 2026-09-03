<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * A blob from the template, used when no value is supplied.
 */
final readonly class BlobFallback
{
    /**
     * @param string $file File in the template holding the blob
     * @param array<string, mixed>|null $meta Metadata passed to the template along with the blob
     */
    public function __construct(
        public string $file,
        public ?array $meta
    ) {
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        return new self($data['file'], $data['meta']);
    }
}
