<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * An input taking a JSON value.
 */
final readonly class JsonInputDefinition implements InputDefinition
{
    /**
     * @param string $key Key the input is supplied and used under
     * @param bool $required Whether a value of this input is required for compilation
     * @param string|null $default File in the template holding the value used when none is supplied;
     *     in development mode the development file takes precedence
     * @param string|null $development File in the template holding the value used in development mode
     *     when none is supplied
     * @param string|null $schema File in the template holding the JSON schema of this input
     * @param bool $validate Whether values are validated against the schema
     */
    public function __construct(
        public string $key,
        public bool $required,
        public ?string $default,
        public ?string $development,
        public ?string $schema,
        public bool $validate
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
            $data['default'],
            $data['development'],
            $data['schema'],
            $data['validate']
        );
    }
}
