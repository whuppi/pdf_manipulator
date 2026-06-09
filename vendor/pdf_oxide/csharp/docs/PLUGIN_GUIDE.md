# PDF Oxide C# Plugin System Guide

## Overview

The PDF Oxide Plugin System provides a wrapper-based extensible architecture for customizing PDF content extraction, searching, and annotation processing. Users can create custom plugins by extending the provided base classes.

**Design Pattern**: Wrapper-based inheritance using C# class extension

**Key Benefits**:
- Simple to understand and extend
- Leverages existing manager patterns
- Full plugin lifecycle management
- Built-in configuration and metrics tracking
- No complex hook infrastructure needed

## Plugin System Architecture

### Base Plugin Classes

The plugin system provides three base plugin classes:

```
PluginBase (virtual methods & lifecycle)
├── ExtractionPlugin (extends ExtractionManager)
├── SearchPlugin (extends SearchManager)
└── AnnotationPlugin (extends AnnotationManager)
```

### Plugin Registry

Central registry for plugin management:

```csharp
var registry = new PluginRegistry();
registry.Register("myPlugin", typeof(MyPluginClass));
var plugin = registry.CreateInstance("myPlugin", document);
```

## Creating Custom Plugins

### 1. Extraction Plugin

Extend `ExtractionPlugin` to customize text extraction behavior.

#### Basic Example

```csharp
using PdfOxide.Core;
using PdfOxide.Plugins;

class TextNormalizationPlugin : ExtractionPlugin
{
    public TextNormalizationPlugin(PdfDocument document)
        : base(document)
    {
    }

    public override string ExtractText(int pageIndex)
    {
        string text = base.ExtractText(pageIndex);
        // Custom normalization
        return System.Text.RegularExpressions.Regex.Replace(
            text.Trim().ToLower(),
            @"\s+",
            " "
        );
    }
}

// Usage
using (var doc = PdfDocument.Open("file.pdf"))
{
    var plugin = new TextNormalizationPlugin(doc);
    string normalizedText = plugin.ExtractText(0);
}
```

#### Advanced Example with Configuration

```csharp
class SmartExtractionPlugin : ExtractionPlugin
{
    public SmartExtractionPlugin(PdfDocument document, Dictionary<string, object> options)
        : base(document, options)
    {
    }

    protected override void OnInitialize(PdfDocument document)
    {
        // Initialize plugin
        SetConfig("preserveFormatting", GetConfig("preserveFormatting", true));
        SetConfig("extractImages", GetConfig("extractImages", false));
        Log("info", "SmartExtraction initialized");
    }

    public override string ExtractText(int pageIndex)
    {
        bool preserveFormatting = (bool)GetConfig("preserveFormatting", true);

        string text = base.ExtractText(pageIndex);

        if (preserveFormatting)
        {
            return text; // Keep original formatting
        }
        else
        {
            return NormalizeText(text);
        }
    }

    private string NormalizeText(string text)
    {
        return System.Text.RegularExpressions.Regex.Replace(
            text.Trim(),
            @"\s+",
            " "
        );
    }
}

// Usage with registry
var registry = new PluginRegistry();
var options = new Dictionary<string, object>
{
    { "preserveFormatting", false }
};

registry.Register("smartExtract", typeof(SmartExtractionPlugin));
var plugin = (ExtractionPlugin)registry.CreateInstance(
    "smartExtract",
    document,
    options
);
```

### 2. Search Plugin

Extend `SearchPlugin` to customize search behavior.

#### Basic Example - Result Filtering

```csharp
using PdfOxide.Plugins;
using System.Collections.Generic;
using System.Linq;

class SearchFilterPlugin : SearchPlugin
{
    public SearchFilterPlugin(PdfDocument document)
        : base(document)
    {
    }

    // Override search method and filter results
}
```

#### Advanced Example - Result Ranking

```csharp
class SearchRankingPlugin : SearchPlugin
{
    public SearchRankingPlugin(PdfDocument document, Dictionary<string, object> options)
        : base(document, options)
    {
    }

    protected override void OnInitialize(PdfDocument document)
    {
        SetConfig("sortByPosition", true);
        Log("info", "SearchRanking initialized");
    }
}
```

### 3. Annotation Plugin

Extend `AnnotationPlugin` to customize annotation processing.

#### Basic Example - Annotation Filtering

```csharp
using PdfOxide.Plugins;
using System.Collections.Generic;

class HighlightOnlyPlugin : AnnotationPlugin
{
    public HighlightOnlyPlugin(PdfPage page)
        : base(page)
    {
    }

    // Override GetAnnotations to filter annotations
}
```

#### Advanced Example - Annotation Enrichment

```csharp
class AnnotationEnrichmentPlugin : AnnotationPlugin
{
    public AnnotationEnrichmentPlugin(PdfPage page, Dictionary<string, object> options)
        : base(page, options)
    {
    }

    protected override void OnInitialize(PdfPage page)
    {
        Log("info", "AnnotationEnrichment started");
    }
}
```

## Plugin Lifecycle

### Lifecycle Hooks

Plugins support initialization and cleanup hooks:

```csharp
class MyPlugin : ExtractionPlugin
{
    protected override void OnInitialize(PdfDocument document)
    {
        // Called when plugin is instantiated
        SetConfig("initialized", true);
        Log("info", "Plugin initialized");
    }

    protected override void OnDestroy()
    {
        // Called when plugin is released
        Log("info", "Plugin destroyed");
    }
}
```

## Plugin Configuration

### Getting and Setting Configuration

```csharp
plugin.SetConfig("maxResults", 100);
plugin.SetConfig("timeout", 30000);

object maxResults = plugin.GetConfig("maxResults"); // 100
object timeout = plugin.GetConfig("timeout");       // 30000
object unknown = plugin.GetConfig("unknown", 50);   // 50 (default)
```

### Configuration with Registry

```csharp
var options = new Dictionary<string, object>
{
    { "maxResults", 50 },
    { "cacheResults", true }
};

registry.Register("mySearchPlugin", typeof(MySearchPlugin));
var plugin = registry.CreateInstance("mySearchPlugin", document, options);
```

## Plugin Logging

### Logging API

```csharp
// Log with level
plugin.Log("info", "Processing started");
plugin.Log("debug", "Debug information");
plugin.Log("warn", "Warning message");
plugin.Log("error", "Error occurred");

// Log with additional data
var data = new Dictionary<string, object>
{
    { "pageCount", 100 },
    { "processingTime", 1234 }
};
plugin.Log("info", "Processing complete", data);
```

### Logging Output

Logs are:
1. Printed to console (info/debug/warn to stdout, error to stderr)
2. Stored in metrics for later retrieval

```csharp
var metrics = plugin.Metrics();
var logs = (List<string>)metrics["logs"];
foreach (var log in logs)
{
    Console.WriteLine(log);
}
```

## Performance Metrics

### Tracking Metrics

All plugins automatically track performance metrics:

```csharp
var metrics = plugin.Metrics();

// Available metrics depend on plugin type
// ExtractionPlugin: extractCalls, totalExtractTime, avgExtractTime
// SearchPlugin: searchCalls, totalSearchTime, avgSearchTime
// AnnotationPlugin: annotationCalls, totalAnnotationTime, avgAnnotationTime
```

### Example: Performance Monitoring

```csharp
var plugin = new ExtractionPlugin(doc);

// Perform extractions
for (int i = 0; i < doc.PageCount; i++)
{
    plugin.ExtractText(i);
}

// Check metrics
var metrics = plugin.Metrics();
long calls = (long)metrics["extractCalls"];
long totalTime = (long)metrics["totalExtractTime"];
double avgTime = (double)metrics["avgExtractTime"];

Console.WriteLine($"Calls: {calls}, Total: {totalTime}ms, Avg: {avgTime:F2}ms");
```

## Plugin Registry

### Basic Registration

```csharp
var registry = new PluginRegistry();

// Register with defaults
registry.Register("myPlugin", typeof(MyPluginClass));

// Register with metadata
var metadata = new Dictionary<string, object>
{
    { "version", "1.0.0" },
    { "category", "extraction" },
    { "description", "My custom plugin" }
};
registry.Register("myPlugin", typeof(MyPluginClass), metadata);
```

### Discovery and Querying

```csharp
// Check if registered
if (registry.IsRegistered("myPlugin"))
{
    // Get all plugins
    var plugins = registry.GetPlugins();

    // Get by category
    var extractors = registry.GetPluginsByCategory("extraction");

    // Get plugin info
    var info = registry.GetPluginInfo("myPlugin");
    Console.WriteLine($"Version: {info.Version}");
    Console.WriteLine($"Category: {info.Category}");
}
```

### Instance Management

```csharp
// Create instance
var plugin = registry.CreateInstance("myPlugin", document);

// Retrieve active instances
var instances = registry.GetInstances();
Console.WriteLine($"Active plugins: {instances.Count}");

// Release instance
registry.ReleaseInstance(plugin);
```

### Statistics and Validation

```csharp
// Get registry statistics
var stats = registry.GetStatistics();
Console.WriteLine($"Registered: {stats["registeredCount"]}");
Console.WriteLine($"Active: {stats["activeInstances"]}");

// Validate configuration
var config = new Dictionary<string, object>
{
    { "option1", "value1" }
};
var validation = registry.ValidatePluginConfig("myPlugin", config);

if (validation.IsValid)
{
    Console.WriteLine($"Config valid: {validation.Message}");
}
else
{
    Console.WriteLine($"Config invalid: {validation.Message}");
}
```

## Complete Example: Multi-Plugin Pipeline

```csharp
using System;
using System.Collections.Generic;
using PdfOxide.Core;
using PdfOxide.Plugins;

class PluginPipelineExample
{
    static void Main(string[] args)
    {
        // Open document
        using (var doc = PdfDocument.Open("document.pdf"))
        {
            // Create registry and register plugins
            var registry = new PluginRegistry();

            // Register extraction plugin
            var extractMeta = new Dictionary<string, object>
            {
                { "category", "extraction" }
            };
            registry.Register("normalizer", typeof(TextNormalizationPlugin), extractMeta);

            // Register search plugin
            var searchMeta = new Dictionary<string, object>
            {
                { "category", "search" }
            };
            registry.Register("ranker", typeof(SearchRankingPlugin), searchMeta);

            // Create plugin instances
            var extractor = (ExtractionPlugin)registry.CreateInstance("normalizer", doc);
            var searcher = (SearchPlugin)registry.CreateInstance("ranker", doc);

            // Use plugins
            string text = extractor.ExtractText(0);
            Console.WriteLine($"Extracted: {text}");

            // Check metrics
            var extractMetrics = extractor.Metrics();
            Console.WriteLine($"Extraction time: {extractMetrics["avgExtractTime"]}ms");

            // Get registry info
            Console.WriteLine($"Registered plugins: {registry.GetPlugins().Count}");
            Console.WriteLine($"Active instances: {registry.GetInstances().Count}");

            // Cleanup
            registry.ReleaseInstance(extractor);
            registry.ReleaseInstance(searcher);
        }
    }
}
```

## C# Best Practices

### 1. Use Properties Where Appropriate

```csharp
class MyPlugin : ExtractionPlugin
{
    public int MaxResults { get; set; } = 100;

    protected override void OnInitialize(PdfDocument document)
    {
        SetConfig(nameof(MaxResults), MaxResults);
    }
}
```

### 2. Leverage LINQ for Collections

```csharp
class SearchFilterPlugin : SearchPlugin
{
    // Use LINQ to filter and process results
}
```

### 3. Use Dictionary Initializers

```csharp
var options = new Dictionary<string, object>
{
    { "maxResults", 50 },
    { "timeout", 30000 },
    { "cacheResults", true }
};
```

### 4. Implement IDisposable if Needed

```csharp
class ResourceIntensivePlugin : ExtractionPlugin, IDisposable
{
    protected override void OnDestroy()
    {
        // Cleanup resources
    }

    public void Dispose()
    {
        OnDestroy();
        GC.SuppressFinalize(this);
    }
}
```

### 5. Use Named Parameters

```csharp
plugin.Log(
    level: "info",
    message: "Processing complete",
    data: new Dictionary<string, object> { { "time", 1234 } }
);
```

## Plugin Registry Best Practices

### 1. Use Categories for Organization

```csharp
registry.Register(
    "plugin1",
    typeof(Plugin1),
    new Dictionary<string, object>
    {
        { "category", "extraction" },
        { "version", "1.0.0" }
    }
);

var extractors = registry.GetPluginsByCategory("extraction");
```

### 2. Validate Before Creating Instances

```csharp
var validation = registry.ValidatePluginConfig("myPlugin", config);

if (validation.IsValid)
{
    var plugin = registry.CreateInstance("myPlugin", doc, config);
}
else
{
    Console.Error.WriteLine($"Invalid config: {validation.Message}");
}
```

### 3. Monitor Active Instances

```csharp
var instances = registry.GetInstances();
Console.WriteLine($"Active instances: {instances.Count}");

// Release unused instances
foreach (var instance in instances)
{
    registry.ReleaseInstance(instance);
}
```

### 4. Use Try-Finally for Cleanup

```csharp
var plugin = registry.CreateInstance("myPlugin", doc);
try
{
    // Use plugin
}
finally
{
    registry.ReleaseInstance(plugin);
}
```

## Troubleshooting

### Plugin Not Registered
```
Error: Plugin "myPlugin" is not registered
```
**Solution**: Call `registry.Register()` before `CreateInstance()`

### Configuration Not Applied
```
Configuration is set but not used
```
**Solution**: Call `GetConfig()` with proper key names and check defaults

### Metrics Not Tracking
```
Metrics show zero calls
```
**Solution**: Ensure plugin methods (ExtractText, Search, etc.) are actually called

## Summary

The C# Plugin System provides:

✅ Simple wrapper-based inheritance
✅ Full lifecycle management (virtual methods)
✅ Configuration and logging
✅ Performance metrics
✅ Plugin registry with discovery
✅ Instance lifecycle management
✅ Type-safe plugin API
✅ C# idioms (properties, LINQ, IDisposable)

See [Plugin System Architecture](./PLUGIN_ARCHITECTURE.md) for more details on cross-language parity.
