<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * The Oicana configuration of a template.
 */
final readonly class OicanaConfig
{
    /**
     * @param int $manifestVersion Version of the manifest format
     * @param list<InputDefinition> $inputs The inputs the template declares, in manifest order
     * @param bool $validateJsonInputsByDefault Whether JSON inputs are validated against their schemas by default
     * @param ExportConfig $export How compiled documents are exported
     * @param FontConfig $fonts Fonts the template expects from its host
     */
    public function __construct(
        public int $manifestVersion,
        public array $inputs,
        public bool $validateJsonInputsByDefault,
        public ExportConfig $export,
        public FontConfig $fonts
    ) {
    }

    /**
     * @param array<string, mixed> $data
     */
    public static function fromArray(array $data): self
    {
        $inputs = array_map(
            static fn (array $input): InputDefinition => $input['type'] === 'json'
                ? JsonInputDefinition::fromArray($input)
                : BlobInputDefinition::fromArray($input),
            $data['inputs']
        );

        return new self(
            $data['manifestVersion'],
            array_values($inputs),
            $data['validateJsonInputsByDefault'],
            ExportConfig::fromArray($data['export']),
            new FontConfig($data['fonts']['require'])
        );
    }
}
