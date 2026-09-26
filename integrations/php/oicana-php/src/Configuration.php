<?php

declare(strict_types=1);

namespace Oicana;

/**
 * Global Oicana configuration.
 */
final class Configuration
{
    private function __construct()
    {
    }

    /**
     * Configure the coloring of compilation diagnostics like warnings and errors.
     */
    public static function configureDiagnosticColor(DiagnosticColor $color): void
    {
        \OicanaInternal\configure_diagnostic_color($color === DiagnosticColor::Ansi);
    }

    /**
     * Configure automatic cache eviction after each compilation.
     *
     * @param int|null $maxAge Maximum age threshold, or null to disable:
     *   - null - Disables cache eviction (cache never cleared)
     *   - 0 - Clears all cache entries with every eviction
     *   - 1 - Keeps only entries used since the last eviction
     *   - n - Keeps entries used within the last n evictions
     *   Default is 10.
     */
    public static function configureAutomaticCacheEviction(?int $maxAge): void
    {
        \OicanaInternal\configure_automatic_cache_eviction($maxAge);
    }

    /**
     * Manually evict the cache with the given age threshold.
     *
     * This directly calls the underlying eviction with the specified age,
     * regardless of the configured default age.
     *
     * @param int $maxAge Maximum age threshold for eviction.
     *   Entries with age >= this value will be removed.
     */
    public static function evictCache(int $maxAge): void
    {
        \OicanaInternal\evict_cache($maxAge);
    }

    /**
     * Make fonts available to every template registered from now on.
     *
     * @param list<string> $fonts Raw content of the font files. Data that holds no
     *                            font is ignored.
     *
     * @return int The number of font faces that were added.
     */
    public static function registerFonts(array $fonts): int
    {
        $faces = 0;
        foreach ($fonts as $font) {
            $faces += \OicanaInternal\register_font($font);
        }

        return $faces;
    }

    /**
     * Make fonts on disk available to every template registered from now on.
     *
     * @param list<string> $paths Paths to font files, or to directories whose font
     *                            files are all added.
     *
     * @return int The number of font faces that were added.
     */
    public static function registerFontPaths(array $paths): int
    {
        return \OicanaInternal\register_font_paths($paths);
    }

    /**
     * All font faces currently registered by the host.
     *
     * @return list<RegisteredFont>
     */
    public static function registeredFonts(): array
    {
        /** @var list<array{family: string, path: string|null}> $fonts */
        $fonts = json_decode(\OicanaInternal\registered_fonts(), true, 512, JSON_THROW_ON_ERROR);

        return array_map(
            static fn (array $font): RegisteredFont => new RegisteredFont($font['family'], $font['path']),
            $fonts,
        );
    }

    /**
     * Drop all fonts registered by the host.
     *
     * Templates that are already registered keep the fonts they were created with.
     */
    public static function clearFonts(): void
    {
        \OicanaInternal\clear_fonts();
    }
}
