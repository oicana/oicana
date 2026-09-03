<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * An input taking arbitrary bytes.
 */
final readonly class BlobInputDefinition implements InputDefinition
{
    /**
     * @param string $key Key the input is supplied and used under
     * @param bool $required Whether a value of this input is required for compilation
     * @param BlobFallback|null $default Blob used when no value is supplied; in development mode the
     *     development blob takes precedence
     * @param BlobFallback|null $development Blob used in development mode when no value is supplied
     */
    public function __construct(
        public string $key,
        public bool $required,
        public ?BlobFallback $default,
        public ?BlobFallback $development
    ) {
    }

    public function key(): string
    {
        return $this->key;
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        return new self(
            $data['key'],
            $data['required'],
            $data['default'] === null ? null : BlobFallback::fromArray($data['default']),
            $data['development'] === null ? null : BlobFallback::fromArray($data['development'])
        );
    }
}
