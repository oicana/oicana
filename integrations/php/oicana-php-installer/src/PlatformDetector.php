<?php

declare(strict_types=1);

namespace Oicana\Installer;

/**
 * Detects the current PHP platform configuration.
 */
final class PlatformDetector
{
    /**
     * Detect the current platform.
     *
     * @throws \RuntimeException If platform cannot be detected
     */
    public function detect(): Platform
    {
        return new Platform(
            os: $this->detectOS(),
            architecture: $this->detectArchitecture(),
            phpVersion: $this->detectPhpVersion(),
            threadSafety: $this->detectThreadSafety()
        );
    }

    /**
     * Detect operating system.
     *
     * @throws \RuntimeException If OS is not supported
     */
    private function detectOS(): string
    {
        return match (PHP_OS_FAMILY) {
            'Windows' => 'windows',
            'Darwin' => 'macos',
            'Linux' => $this->detectLinux(),
            default => throw new \RuntimeException('Unsupported OS: ' . PHP_OS_FAMILY),
        };
    }

    /**
     * Detect Linux, rejecting musl based systems.
     *
     * @throws \RuntimeException If the system uses musl
     */
    private function detectLinux(): string
    {
        if ($this->isMusl()) {
            throw new \RuntimeException(
                'Unsupported platform: there is no musl build of the Oicana extension, and this '
                . 'system uses musl (Alpine and other musl based distributions).'
            );
        }

        return 'linux';
    }

    /**
     * Whether the C library is musl rather than glibc.
     */
    private function isMusl(): bool
    {
        return !empty(glob('/lib/ld-musl-*.so.1')) || !empty(glob('/lib/libc.musl-*.so.1'));
    }

    /**
     * Detect CPU architecture.
     *
     * @throws \RuntimeException If architecture is not supported
     */
    private function detectArchitecture(): string
    {
        $arch = php_uname('m');
        return match (true) {
            str_contains($arch, 'x86_64') || str_contains($arch, 'amd64') || str_contains($arch, 'AMD64') => 'x64',
            str_contains($arch, 'aarch64') || str_contains($arch, 'arm64') => 'arm64',
            default => throw new \RuntimeException('Unsupported architecture: ' . $arch),
        };
    }

    /**
     * Detect PHP major.minor version.
     *
     * @throws \RuntimeException If PHP version cannot be determined
     */
    private function detectPhpVersion(): string
    {
        $version = PHP_VERSION;
        if (preg_match('/^(\d+\.\d+)/', $version, $matches)) {
            return $matches[1];
        }
        throw new \RuntimeException('Could not detect PHP version from: ' . $version);
    }

    /**
     * Detect thread safety mode.
     */
    private function detectThreadSafety(): string
    {
        return ZEND_THREAD_SAFE ? 'zts' : 'nts';
    }
}
