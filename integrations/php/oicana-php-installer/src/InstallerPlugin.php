<?php

declare(strict_types=1);

namespace Oicana\Installer;

use Composer\Composer;
use Composer\EventDispatcher\EventSubscriberInterface;
use Composer\IO\IOInterface;
use Composer\Plugin\PluginInterface;
use Composer\Script\Event;
use Composer\Script\ScriptEvents;

/**
 * Composer plugin that automatically downloads the Oicana native extension.
 *
 * This plugin hooks into Composer's post-install and post-update events
 * to download the appropriate native extension binary for the current platform
 * and write a PHP ini file that activates it via PHP_INI_SCAN_DIR.
 */
final class InstallerPlugin implements PluginInterface, EventSubscriberInterface
{
    private const PACKAGE_NAME = 'oicana/installer';

    private ?Composer $composer = null;

    /**
     * {@inheritdoc}
     */
    public function activate(Composer $composer, IOInterface $io): void
    {
        $this->composer = $composer;
    }

    /**
     * {@inheritdoc}
     */
    public function deactivate(Composer $composer, IOInterface $io): void
    {
    }

    /**
     * {@inheritdoc}
     */
    public function uninstall(Composer $composer, IOInterface $io): void
    {
    }

    /**
     * {@inheritdoc}
     *
     * @return array<string, string|array{0: string, 1?: int}>
     */
    public static function getSubscribedEvents(): array
    {
        return [
            ScriptEvents::POST_INSTALL_CMD => 'installExtension',
            ScriptEvents::POST_UPDATE_CMD => 'installExtension',
        ];
    }

    /**
     * Install the Oicana native extension.
     *
     * This method is called after `composer install` or `composer update`.
     */
    public function installExtension(Event $event): void
    {
        $io = $event->getIO();

        try {
            $io->write('<info>Installing Oicana native extension...</info>');

            $detector = new PlatformDetector();
            $platform = $detector->detect();

            $io->write(sprintf(
                '<info>Detected platform: %s</info>',
                $platform->getDescription()
            ));

            $version = $this->resolveOwnVersion();
            $downloader = new BinaryDownloader($version, $this->composer->getLoop()->getHttpDownloader());
            $extensionPath = $downloader->download($platform);

            $io->write('<info>✓ Extension downloaded to: ' . $extensionPath . '</info>');

            $iniDir = $this->writeIniFile($extensionPath);

            $io->write('<info>✓ PHP ini file written to: ' . $iniDir . DIRECTORY_SEPARATOR . 'oicana.ini</info>');

            $this->showActivationInstructions($io, $iniDir);

        } catch (UnsupportedPlatformException $e) {
            $io->writeError('<warning>Oicana extension not installed: ' . $e->getMessage() . '</warning>');
            $io->writeError('');
            $io->writeError('<comment>To install the extension manually, see:</comment>');
            $io->writeError('<comment>https://github.com/oicana/oicana/tree/main/integrations/php/oicana-php-installer#if-automatic-installation-fails</comment>');
            $io->writeError('');
        }
    }

    /**
     * Write a PHP ini file that loads the extension from its vendor path.
     *
     * @return string The directory containing the written ini file
     */
    private function writeIniFile(string $extensionPath): string
    {
        $iniDir = implode(DIRECTORY_SEPARATOR, [
            getcwd(),
            'vendor',
            'oicana',
            'installer',
            'php',
        ]);

        if (!is_dir($iniDir)) {
            mkdir($iniDir, 0755, true);
        }

        $iniPath = $iniDir . DIRECTORY_SEPARATOR . 'oicana.ini';
        file_put_contents($iniPath, 'extension=' . $extensionPath . PHP_EOL);

        return $iniDir;
    }

    /**
     * Show instructions for activating the extension via PHP_INI_SCAN_DIR.
     */
    private function showActivationInstructions(IOInterface $io, string $iniDir): void
    {
        $io->write('');
        $io->write('<comment>To activate the extension, set this environment variable before starting PHP:</comment>');
        $io->write('');

        if (PHP_OS_FAMILY === 'Windows') {
            $io->write(sprintf('<comment>  set PHP_INI_SCAN_DIR=";%s"</comment>', $iniDir));
        } else {
            $io->write(sprintf('<comment>  export PHP_INI_SCAN_DIR=":%s"</comment>', $iniDir));
        }

        $io->write('');
        $io->write('<comment>Verify the extension is loaded with:</comment>');
        $io->write('<comment>  php -m | grep oicana</comment>');
        $io->write('');
        $io->write('<comment>You can re-display this command at any time with:</comment>');
        $io->write('<comment>  vendor/bin/oicana-env</comment>');
        $io->write('');
    }

    /**
     * Resolve the version of this plugin from Composer's local repository.
     *
     * Falling back to anything else would defeat the point of this lookup, so
     * a missing package or unresolved version is treated as a hard error.
     */
    private function resolveOwnVersion(): string
    {
        if ($this->composer === null) {
            throw new \RuntimeException('Composer instance not available; activate() was not called');
        }

        // When the installer is being built/tested in its own checkout it is
        // the root package and not present in the local vendor repository.
        $rootPackage = $this->composer->getPackage();
        if ($rootPackage->getName() === self::PACKAGE_NAME) {
            return $rootPackage->getPrettyVersion();
        }

        $package = $this->composer
            ->getRepositoryManager()
            ->getLocalRepository()
            ->findPackage(self::PACKAGE_NAME, '*');

        if ($package === null) {
            throw new \RuntimeException(sprintf(
                'Could not find package "%s" in the local repository',
                self::PACKAGE_NAME
            ));
        }

        return $package->getPrettyVersion();
    }
}
