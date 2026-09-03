<?php

declare(strict_types=1);

use Oicana\CompilationMode;
use Oicana\ExportFormat;
use Oicana\Inputs\BlobInput;
use Oicana\Manifest\BlobFallback;
use Oicana\Manifest\BlobInputDefinition;
use Oicana\Manifest\JsonInputDefinition;
use Oicana\PageRange;
use Oicana\Template;

beforeEach(function () {
    if (!extension_loaded('oicana')) {
        throw new RuntimeException(
            'oicana extension not loaded. Load it with: php -d extension=/path/to/liboicana_php_native.so'
        );
    }

    if (!file_exists(e2e_template_path())) {
        throw new RuntimeException(
            'E2E template not found at ' . e2e_template_path() . '. Run `oicana pack` in e2e-tests/template first.'
        );
    }

    // Ensure testOutput directory exists
    $outputDir = __DIR__ . '/testOutput';
    if (!is_dir($outputDir)) {
        mkdir($outputDir, 0755, true);
    }
});

test('e2e development', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    try {
        $image = $template->export(
            exportFormat: ExportFormat::png(pixelsPerPt: 1.0),
            mode: CompilationMode::Development
        );

        file_put_contents(__DIR__ . '/testOutput/development.png', $image);

        expect($image)
            ->not->toBeEmpty()
            ->and($image)->toStartWith("\x89PNG");
    } finally {
        $template->cleanup();
    }
});

test('e2e production', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    $blob = file_get_contents(assets_path('inputs/input.txt'));
    $json = file_get_contents(assets_path('inputs/input.json'));
    assert(is_string($blob) && is_string($json));

    $blobInputs = [
        'development-blob' => new BlobInput($blob, [
            'image_format' => 'jpeg',
            'foo' => 43,
            'bar' => ['input', 'two'],
        ]),
    ];
    $jsonInputs = [
        'development-json' => $json,
    ];

    try {
        $image = $template->export(
            jsonInputs: $jsonInputs,
            blobInputs: $blobInputs,
            exportFormat: ExportFormat::png(pixelsPerPt: 1.0)
        );

        file_put_contents(__DIR__ . '/testOutput/production.png', $image);

        expect($image)
            ->not->toBeEmpty()
            ->and($image)->toStartWith("\x89PNG");
    } finally {
        $template->cleanup();
    }
});

test('e2e all-inputs', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    $blob = file_get_contents(assets_path('inputs/input.txt'));
    $json = file_get_contents(assets_path('inputs/input.json'));
    assert(is_string($blob) && is_string($json));

    $blobInputs = [
        'default-blob' => new BlobInput($blob, [
            'image_format' => 'jpeg',
            'foo' => 42,
            'bar' => ['input', 'two'],
        ]),
        'development-blob' => new BlobInput($blob, [
            'image_format' => 'jpeg',
            'foo' => 43,
            'bar' => ['input', 'two'],
        ]),
        'both-blob' => new BlobInput($blob, [
            'image_format' => 'jpeg',
            'foo' => 44,
            'bar' => ['input', 'two'],
        ]),
    ];
    $jsonInputs = [
        'default-json' => $json,
        'development-json' => $json,
        'both-json' => $json,
    ];

    try {
        $image = $template->export(
            jsonInputs: $jsonInputs,
            blobInputs: $blobInputs,
            exportFormat: ExportFormat::png(pixelsPerPt: 1.0)
        );

        file_put_contents(__DIR__ . '/testOutput/all-inputs.png', $image);

        expect($image)
            ->not->toBeEmpty()
            ->and($image)->toStartWith("\x89PNG");
    } finally {
        $template->cleanup();
    }
});

test('manifest exposes the package section and the Oicana config', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    try {
        $manifest = $template->manifest();

        expect($manifest->package->name)->toBe('oicana-e2e-test');
        expect($manifest->package->version)->toBe('0.1.0');
        expect($manifest->oicana->manifestVersion)->toBe(1);
        expect($manifest->oicana->validateJsonInputsByDefault)->toBeTrue();
        expect($manifest->oicana->export->pdf->standards)->toBe(['a-3b']);
        expect($manifest->oicana->fonts->require)->toBe([]);

        $json = null;
        $blob = null;
        foreach ($manifest->oicana->inputs as $input) {
            if ($input instanceof JsonInputDefinition && $input->key === 'development-json') {
                $json = $input;
            }
            if ($input instanceof BlobInputDefinition && $input->key === 'default-blob') {
                $blob = $input;
            }
        }

        expect($json)->toBeInstanceOf(JsonInputDefinition::class);
        expect($blob)->toBeInstanceOf(BlobInputDefinition::class);
        assert($json instanceof JsonInputDefinition);
        assert($blob instanceof BlobInputDefinition);

        expect($json->schema)->toBe('input.schema.json');
        expect($json->development)->toBe('development.json');
        expect($json->default)->toBeNull();
        expect($json->validate)->toBeTrue();

        $fallback = $blob->default;
        expect($fallback)->toBeInstanceOf(BlobFallback::class);
        assert($fallback instanceof BlobFallback);
        expect($fallback->file)->toBe('default.txt');
        $meta = $fallback->meta;
        expect($meta)->not->toBeNull();
        assert($meta !== null);
        expect($meta['image_format'])->toBe('png');
        expect($blob->development)->toBeNull();
    } finally {
        $template->cleanup();
    }
});

test('explicit development mode allows compile with empty inputs', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    try {
        $image = $template->export(
            exportFormat: ExportFormat::png(pixelsPerPt: 1.0),
            mode: CompilationMode::Development
        );

        expect($image)->not->toBeEmpty();
    } finally {
        $template->cleanup();
    }
});

test('compile defaults to production mode', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes, mode: CompilationMode::Development);

    try {
        expect(fn() => $template->export(exportFormat: ExportFormat::png(pixelsPerPt: 1.0)))
            ->toThrow(Exception::class, 'No value for the required input');
    } finally {
        $template->cleanup();
    }
});

test('can control compilation mode when registering', function () {
    $templateBytes = file_get_contents(e2e_template_path());

    expect(fn() => new Template($templateBytes, mode: CompilationMode::Production))
        ->toThrow(Exception::class, 'No value for the required input');
});

test('compiled document handle survives template cleanup', function () {
    $templateBytes = file_get_contents(e2e_template_path());
    $template = new Template($templateBytes);

    $document = $template->compile(mode: CompilationMode::Development);

    $template->cleanup();

    expect($document->pageCount())->toBeGreaterThan(0);
    $firstPage = PageRange::single(0);

    $pdf = $document->exportPdf($firstPage);
    expect($pdf)->toStartWith('%PDF');

    $png = $document->export(ExportFormat::png(pixelsPerPt: 1.0), $firstPage);
    expect($png)->toStartWith("\x89PNG");

    $svg = $document->exportSvg($firstPage);
    expect($svg)->toContain('<svg');

    $firstPagePng = $document->exportPng(1.0, PageRange::single(0));
    expect($firstPagePng)->toStartWith("\x89PNG");

    $document->close();
});
