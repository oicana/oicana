namespace Oicana;

/// <summary>
/// A service for using Oicana templates
/// </summary>
public interface IOicanaService : IDisposable
{
    /// <summary>
    /// Register a <see cref="Template"/> for the given id based on the given template file.
    ///
    /// Registering an id that is already in use replaces the previous template and disposes it.
    /// </summary>
    /// <param name="id">Identifier for this registration.</param>
    /// <param name="file">The packed Oicana template file.</param>
    void RegisterTemplate(string id, byte[] file);

    /// <summary>
    /// Register an already created template for the given id.
    ///
    /// Registering an id that is already in use replaces the previous template and disposes it.
    /// </summary>
    /// <param name="id">Identifier for this registration.</param>
    /// <param name="template">The template to register.</param>
    void RegisterTemplate(string id, ITemplate template);

    /// <summary>
    /// Get the template registered under the given id.
    /// </summary>
    /// <param name="id">Identifier of the template.</param>
    /// <returns><see cref="Template"/> if id is registered, <see langword="null"/> if the id is not registered.</returns>
    ITemplate? GetTemplate(string id);

    /// <summary>
    /// Remove the template registered under the given id from the service.
    ///
    /// The caller takes over ownership of the returned template and has to dispose it.
    /// </summary>
    /// <param name="id">Identifier of the template.</param>
    /// <returns><see cref="Template"/> if id is registered, <see langword="null"/> if the id is not registered.</returns>
    ITemplate? RemoveTemplate(string id);
}
