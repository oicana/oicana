using System.Collections.Concurrent;
using System.Diagnostics;
using Microsoft.Extensions.Logging;
using Oicana.Interop;

namespace Oicana;

/// <summary>
/// A service for using Oicana templates
/// </summary>
/// <remarks>
/// This service is thread-safe.
/// You most likely what to keep it around as a singleton.
/// </remarks>
public class OicanaService : IOicanaService
{
    private readonly ConcurrentDictionary<string, ITemplate> _templates;
    private readonly ILogger<OicanaService> _logger;

    /// <summary>
    /// Create a new service.
    /// </summary>
    /// <param name="logger"></param>
    public OicanaService(ILogger<OicanaService> logger)
    {
        _templates = new ConcurrentDictionary<string, ITemplate>();
        _logger = logger;
    }

    /// <inheritdoc />
    public ITemplate? GetTemplate(string id)
    {
        return _templates.GetValueOrDefault(id);
    }

    /// <inheritdoc />
    public ITemplate? RemoveTemplate(string id)
    {
        _templates.Remove(id, out var template);
        return template;
    }

    /// <inheritdoc />
    /// <exception cref="OicanaException">If the initial template compilation fails.</exception>
    public void RegisterTemplate(string id, byte[] file)
    {
        _logger.LogInformation("Registering Oicana template: {Id}", id);
        var stopWatch = new Stopwatch();
        stopWatch.Start();
        var template = new Template(file);
        stopWatch.Stop();
        Store(id, template);
        _logger.LogInformation("Warming up the template '{Id}' took {time}ms", id, stopWatch.ElapsedMilliseconds);
    }

    /// <inheritdoc />
    public void RegisterTemplate(string id, ITemplate template)
    {
        _logger.LogInformation("Registering Oicana template: {Id}", id);
        Store(id, template);
    }

    private void Store(string id, ITemplate template)
    {
        _templates.AddOrUpdate(id, template, (_, replaced) =>
        {
            if (!ReferenceEquals(replaced, template))
            {
                _logger.LogInformation("Replacing Oicana template: {Id}", id);
                replaced.Dispose();
            }

            return template;
        });
    }

    /// <inheritdoc />
    public void Dispose()
    {
        foreach (var id in _templates.Keys)
        {
            if (_templates.TryRemove(id, out var template))
            {
                template.Dispose();
            }
        }
    }
}
