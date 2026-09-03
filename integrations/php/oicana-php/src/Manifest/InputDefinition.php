<?php

declare(strict_types=1);

namespace Oicana\Manifest;

/**
 * An input a template declares.
 *
 * Implemented by {@see JsonInputDefinition} and {@see BlobInputDefinition}.
 */
interface InputDefinition
{
    /**
     * Key the input is supplied and used under.
     */
    public function key(): string;
}
