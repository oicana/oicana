<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * Fonts a template expects from its host.
 */
final readonly class FontConfig
{
    /**
     * @param list<string> $require Font families the host has to register
     */
    public function __construct(
        public array $require
    ) {
    }
}
