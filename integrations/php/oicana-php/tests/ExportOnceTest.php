<?php

declare(strict_types=1);

use Oicana\CompilationMode;
use Oicana\Configuration;
use Oicana\DiagnosticColor;
use Oicana\ExportFormat;
use Oicana\ExportOnceResult;
use Oicana\Template;
use Oicana\ZipLimits;

beforeEach(function () {
    if (!extension_loaded('oicana')) {
        test()->markTestSkipped('oicana extension not loaded');
    }

    if (!file_exists(e2e_template_path())) {
        test()->markTestSkipped('E2E template not found. Run `oicana pack` in e2e-tests/template first.');
    }
});

test('exportOnce exports without warnings', function () {
    $templateBytes = file_get_contents(e2e_template_path());

    $result = Template::exportOnce($templateBytes, mode: CompilationMode::Development);

    expect($result)->toBeInstanceOf(ExportOnceResult::class)
        ->and($result->document)->toStartWith('%PDF')
        ->and($result->warnings)->toBeNull();
});

test('exportOnce surfaces warnings', function () {
    $templateBytes = pack_minimal_template(
        "#set text(font: \"NonexistentFontExportOnce\")\nContent"
    );

    $result = Template::exportOnce(
        $templateBytes,
        exportFormat: ExportFormat::svg(),
        mode: CompilationMode::Development
    );

    expect($result->document)->toContain('<svg')
        ->and($result->warnings)->not->toBeNull()
        ->and($result->warnings)->toContain('NonexistentFontExportOnce');
});

test('exportOnce enforces zip limits', function () {
    $templateBytes = file_get_contents(e2e_template_path());

    expect(fn() => Template::exportOnce(
        $templateBytes,
        mode: CompilationMode::Development,
        limits: new ZipLimits(maxEntries: 1)
    ))->toThrow(Exception::class, 'entries');
});

test('registration enforces zip limits', function () {
    $templateBytes = file_get_contents(e2e_template_path());

    expect(fn() => new Template(
        $templateBytes,
        limits: new ZipLimits(maxEntries: 1)
    ))->toThrow(Exception::class, 'entries');
});

test('diagnostic color configuration succeeds', function () {
    Configuration::configureDiagnosticColor(DiagnosticColor::Ansi);
    Configuration::configureDiagnosticColor(DiagnosticColor::None);

    expect(true)->toBeTrue();
});
