/**
 * Manager for PDF layers (Optional Content Groups - OCG)
 *
 * Provides methods to manage and interact with PDF layers which are used
 * for optional content groups in PDF documents.
 *
 * @example
 * ```typescript
 * import { LayerManager } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const layerManager = new LayerManager(doc);
 *
 * // Check if document has layers
 * if (layerManager.hasLayers()) {
 *   const layers = layerManager.getLayers();
 *   console.log(`Document has ${layers.length} layers`);
 * }
 * ```
 */

export interface Layer {
  id: string;
  name: string;
  visible: boolean;
  index: number;
  parentId: string | null;
  printable: boolean;
  export: boolean;
  description?: string;
  dependsOn?: string[];
}

export interface LayerNode extends Layer {
  children: LayerNode[];
}

export interface LayerHierarchy {
  root: LayerNode[];
}

export interface LayerStatistics {
  count: number;
  rootCount: number;
  maxDepth: number;
  visible: number;
  hidden: number;
  printable: number;
  exportable: number;
  hasConflicts: boolean;
}

export interface LayerValidation {
  isValid: boolean;
  issues: string[];
}

export class LayerManager {
  private _document: any;
  private _layerCache: Layer[] | null;
  private _hierarchyCache: LayerHierarchy | null;
  private _statisticsCache: LayerStatistics | null;

  /**
   * Creates a new LayerManager for the given document
   * @param document - The PDF document
   * @throws Error if document is null or undefined
   */
  constructor(document: any) {
    if (!document) {
      throw new Error('Document is required');
    }
    this._document = document;
    // Performance optimization: cache layer data
    this._layerCache = null;
    this._hierarchyCache = null;
    this._statisticsCache = null;
  }

  /**
   * Clears the layer cache
   * Useful when document content might have changed
   */
  clearCache(): void {
    this._layerCache = null;
    this._hierarchyCache = null;
    this._statisticsCache = null;
  }

  /**
   * Checks if document has layers
   * @returns True if document contains layers
   */
  hasLayers(): boolean {
    try {
      return this.getLayerCount() > 0;
    } catch (error) {
      return false;
    }
  }

  /**
   * Gets number of layers in document
   * @returns Number of layers
   */
  getLayerCount(): number {
    const layers = this.getLayers();
    return layers.length;
  }

  /**
   * Gets all layers in document
   * @returns Array of layer objects with id, name, visible, etc.
   *
   * @example
   * ```typescript
   * const layers = manager.getLayers();
   * layers.forEach(layer => {
   *   console.log(`Layer: ${layer.name} (visible: ${layer.visible})`);
   * });
   * ```
   */
  getLayers(): Layer[] {
    // Performance optimization: cache layer data
    if (this._layerCache !== null) {
      return this._layerCache;
    }

    try {
      const rawLayers = this._document.getLayers();
      // Convert native LayerInfo to the expected JS format
      const layers: Layer[] = rawLayers.map((layer: any) => ({
        id: layer.id,
        name: layer.name,
        visible: layer.visible,
        index: layer.index,
        parentId: null, // OCGs don't have hierarchy in PDF spec
        printable: true, // Default assumption
        export: true, // Default assumption
      }));
      this._layerCache = layers;
      return layers;
    } catch (error) {
      return [];
    }
  }

  /**
   * Gets layer by name
   * @param name - Layer name to find
   * @returns Layer object or null if not found
   *
   * @example
   * ```typescript
   * const layer = manager.getLayerByName('Background');
   * if (layer) {
   *   console.log(`Found layer: ${layer.name}`);
   * }
   * ```
   */
  getLayerByName(name: string): Layer | null {
    if (!name || typeof name !== 'string') {
      throw new Error('Layer name must be a non-empty string');
    }

    const layers = this.getLayers();
    return layers.find((layer) => layer.name === name) || null;
  }

  /**
   * Gets layer by ID
   * @param id - Layer ID to find
   * @returns Layer object or null if not found
   */
  getLayerById(id: string): Layer | null {
    if (!id || typeof id !== 'string') {
      throw new Error('Layer ID must be a non-empty string');
    }

    const layers = this.getLayers();
    return layers.find((layer) => layer.id === id) || null;
  }

  /**
   * Gets root-level layers (not nested under other layers)
   * @returns Array of root-level layers
   */
  getRootLayers(): Layer[] {
    const layers = this.getLayers();
    return layers.filter((layer) => !layer.parentId);
  }

  /**
   * Gets the full layer hierarchy as a tree structure
   * @returns Layer hierarchy tree
   *
   * @example
   * ```typescript
   * const hierarchy = manager.getLayerHierarchy();
   * // { root: [{ name: 'Layer1', children: [...] }, ...] }
   * ```
   */
  getLayerHierarchy(): LayerHierarchy {
    // Performance optimization: cache hierarchy
    if (this._hierarchyCache !== null) {
      return this._hierarchyCache;
    }

    const layers = this.getLayers();
    const hierarchy: LayerHierarchy = { root: [] };

    // Build parent-child relationships
    const layerMap = new Map<string, LayerNode>(
      layers.map((l) => [l.id, { ...l, children: [] } as LayerNode])
    );

    for (const layer of layerMap.values()) {
      if (layer.parentId) {
        const parent = layerMap.get(layer.parentId);
        if (parent) {
          parent.children.push(layer);
        }
      } else {
        hierarchy.root.push(layer);
      }
    }

    this._hierarchyCache = hierarchy;
    return hierarchy;
  }

  /**
   * Gets child layers of a parent layer
   * @param parentId - Parent layer ID
   * @returns Array of child layers
   */
  getChildLayers(parentId: string): Layer[] {
    if (!parentId || typeof parentId !== 'string') {
      throw new Error('Parent layer ID must be a non-empty string');
    }

    const layers = this.getLayers();
    return layers.filter((layer) => layer.parentId === parentId);
  }

  /**
   * Gets parent layer of a layer
   * @param layerId - Layer ID
   * @returns Parent layer object or null if no parent
   */
  getParentLayer(layerId: string): Layer | null {
    if (!layerId || typeof layerId !== 'string') {
      throw new Error('Layer ID must be a non-empty string');
    }

    const layer = this.getLayerById(layerId);
    if (!layer || !layer.parentId) {
      return null;
    }

    return this.getLayerById(layer.parentId);
  }

  /**
   * Checks if a layer is visible
   * @param layerId - Layer ID
   * @returns True if layer is visible
   */
  isLayerVisible(layerId: string): boolean {
    const layer = this.getLayerById(layerId);
    return layer ? layer.visible !== false : false;
  }

  /**
   * Gets visibility chain from root to layer
   * Shows visibility state of all parent layers
   * @param layerId - Layer ID
   * @returns Array of layers from root to target
   */
  getVisibilityChain(layerId: string): Layer[] {
    const chain: Layer[] = [];
    let current = this.getLayerById(layerId);

    while (current) {
      chain.unshift(current);
      current = current.parentId ? this.getLayerById(current.parentId) : null;
    }

    return chain;
  }

  /**
   * Gets layer usage information
   * @returns Layer usage { view, print, export }
   */
  getLayerUsages(): Record<string, number> {
    const layers = this.getLayers();
    return {
      view: layers.filter((l) => l.printable === false).length,
      print: layers.filter((l) => l.printable === true).length,
      export: layers.filter((l) => l.export !== false).length,
    };
  }

  /**
   * Gets statistics about layers
   * @returns Layer statistics
   *
   * @example
   * ```typescript
   * const stats = manager.getLayerStatistics();
   * console.log(`Total layers: ${stats.count}`);
   * console.log(`Max depth: ${stats.maxDepth}`);
   * ```
   */
  getLayerStatistics(): LayerStatistics {
    // Performance optimization: cache statistics
    if (this._statisticsCache !== null) {
      return this._statisticsCache;
    }

    const layers = this.getLayers();
    const hierarchy = this.getLayerHierarchy();

    // Calculate max depth
    const calculateDepth = (node: any, depth: number = 0): number => {
      if (!node.children || node.children.length === 0) {
        return depth;
      }
      return Math.max(...node.children.map((child: any) => calculateDepth(child, depth + 1)));
    };

    let maxDepth = 0;
    for (const rootLayer of hierarchy.root) {
      maxDepth = Math.max(maxDepth, calculateDepth(rootLayer));
    }

    const stats: LayerStatistics = {
      count: layers.length,
      rootCount: hierarchy.root.length,
      maxDepth,
      visible: layers.filter((l) => l.visible !== false).length,
      hidden: layers.filter((l) => l.visible === false).length,
      printable: layers.filter((l) => l.printable !== false).length,
      exportable: layers.filter((l) => l.export !== false).length,
      hasConflicts: this._detectLayerConflicts().length > 0,
    };

    this._statisticsCache = stats;
    return stats;
  }

  /**
   * Gets layer dependencies
   * @returns Layer dependencies map
   * @private
   */
  private getLayerDependencies(): Record<string, any> {
    const layers = this.getLayers();
    const dependencies: Record<string, any> = {};

    layers.forEach((layer) => {
      dependencies[layer.id] = {
        dependsOn: layer.dependsOn || [],
        dependents: [],
      };
    });

    // Build reverse dependencies
    Object.entries(dependencies).forEach(([layerId, deps]) => {
      deps.dependsOn.forEach((depId: string) => {
        if (dependencies[depId]) {
          dependencies[depId].dependents.push(layerId);
        }
      });
    });

    return dependencies;
  }

  /**
   * Finds layers by pattern
   * @param pattern - Pattern to match
   * @returns Matching layers
   *
   * @example
   * ```typescript
   * const backgroundLayers = manager.findLayersByPattern(/background/i);
   * ```
   */
  findLayersByPattern(pattern: RegExp | string): Layer[] {
    if (!pattern) {
      throw new Error('Pattern must be provided');
    }

    const regex = pattern instanceof RegExp ? pattern : new RegExp(pattern, 'i');
    const layers = this.getLayers();

    return layers.filter(
      (layer) => regex.test(layer.name) || (layer.description && regex.test(layer.description))
    );
  }

  /**
   * Validates layer state for conflicts and issues
   * @returns Validation result { isValid, issues }
   */
  validateLayerState(): LayerValidation {
    const issues: string[] = [];
    const conflicts = this._detectLayerConflicts();
    const cycles = this._detectLayerCycles();

    if (conflicts.length > 0) {
      issues.push(...conflicts.map((c) => `Conflict: ${c}`));
    }

    if (cycles.length > 0) {
      issues.push(...cycles.map((c) => `Cycle detected: ${c}`));
    }

    return {
      isValid: issues.length === 0,
      issues,
    };
  }

  /**
   * Detects layer conflicts
   * @returns Array of conflict descriptions
   * @private
   */
  private _detectLayerConflicts(): string[] {
    const conflicts: string[] = [];
    const layers = this.getLayers();

    // Check for layers with same name
    const nameMap = new Map<string, string>();
    layers.forEach((layer) => {
      if (nameMap.has(layer.name)) {
        conflicts.push(`Duplicate layer name: ${layer.name}`);
      }
      nameMap.set(layer.name, layer.id);
    });

    // Check for orphaned layers
    const parentIds = new Set(layers.map((l) => l.parentId).filter((id) => id));
    const layerIds = new Set(layers.map((l) => l.id));

    parentIds.forEach((parentId) => {
      if (!layerIds.has(parentId as string)) {
        conflicts.push(`Orphaned layer reference: ${parentId}`);
      }
    });

    return conflicts;
  }

  /**
   * Detects cycles in layer hierarchy
   * @returns Array of cycle descriptions
   * @private
   */
  private _detectLayerCycles(): string[] {
    const cycles: string[] = [];
    const layers = this.getLayers();
    const visited = new Set<string>();
    const stack = new Set<string>();

    const detectCycle = (layerId: string, path: string[] = []): void => {
      if (stack.has(layerId)) {
        cycles.push(`Cycle detected: ${path.join(' -> ')} -> ${layerId}`);
        return;
      }

      if (visited.has(layerId)) {
        return;
      }

      visited.add(layerId);
      stack.add(layerId);
      path.push(layerId);

      const layer = this.getLayerById(layerId);
      if (layer && layer.parentId) {
        detectCycle(layer.parentId, [...path]);
      }

      stack.delete(layerId);
    };

    layers.forEach((layer) => {
      if (!visited.has(layer.id)) {
        detectCycle(layer.id);
      }
    });

    return cycles;
  }
}
