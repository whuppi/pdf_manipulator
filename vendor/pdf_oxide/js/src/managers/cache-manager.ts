/**
 * Cache Manager - TypeScript/Node.js Implementation
 *
 * Provides caching functionality for PDF operations:
 * - Document-level caching
 * - Page-level caching
 * - Result caching with TTL
 * - LRU eviction
 * - Cache statistics
 *
 * This completes the cache coverage for 100% FFI parity.
 */

import { EventEmitter } from 'events';

// ============================================================================
// Type Definitions
// ============================================================================

/**
 * Cache entry with metadata
 */
export interface CacheEntry<T = unknown> {
  readonly key: string;
  readonly value: T;
  readonly timestamp: number;
  readonly ttl?: number;
  readonly size?: number;
  readonly hits: number;
}

/**
 * Cache options
 */
export interface CacheOptions {
  readonly maxSize?: number;
  readonly maxEntries?: number;
  readonly defaultTtl?: number;
  readonly enableStatistics?: boolean;
  readonly evictionPolicy?: 'lru' | 'lfu' | 'fifo';
}

/**
 * Cache statistics
 */
export interface CacheStatistics {
  readonly totalEntries: number;
  readonly totalSize: number;
  readonly hitCount: number;
  readonly missCount: number;
  readonly hitRate: number;
  readonly evictionCount: number;
  readonly oldestEntry?: number;
  readonly newestEntry?: number;
}

/**
 * Cache scope enumeration
 */
export enum CacheScope {
  /** Cache per document */
  DOCUMENT = 'document',
  /** Cache per page */
  PAGE = 'page',
  /** Global cache */
  GLOBAL = 'global',
  /** Session-level cache */
  SESSION = 'session',
}

/**
 * Cache event data
 */
export interface CacheEventData {
  readonly key: string;
  readonly scope?: CacheScope;
  readonly operation: 'set' | 'get' | 'delete' | 'evict' | 'clear';
  readonly hit?: boolean;
}

// ============================================================================
// Cache Manager Implementation
// ============================================================================

/**
 * Cache Manager - Complete caching capabilities
 *
 * Provides 10 functions for cache management:
 * 1. set - Store a value
 * 2. get - Retrieve a value
 * 3. has - Check if key exists
 * 4. delete - Remove a key
 * 5. clear - Clear all entries
 * 6. clearScope - Clear entries by scope
 * 7. getStatistics - Get cache statistics
 * 8. setTtl - Set TTL for an entry
 * 9. getKeys - Get all keys
 * 10. prune - Remove expired entries
 *
 * @example
 * ```typescript
 * const cache = new CacheManager({
 *   maxEntries: 1000,
 *   defaultTtl: 60000, // 1 minute
 *   evictionPolicy: 'lru',
 * });
 *
 * // Store a value
 * cache.set('page:0:text', extractedText, CacheScope.PAGE);
 *
 * // Retrieve a value
 * const text = cache.get<string>('page:0:text');
 *
 * // Check statistics
 * const stats = cache.getStatistics();
 * console.log(`Hit rate: ${stats.hitRate * 100}%`);
 * ```
 */
export class CacheManager extends EventEmitter {
  private readonly cache: Map<string, CacheEntry> = new Map();
  private readonly scopeIndex: Map<CacheScope, Set<string>> = new Map();
  private readonly options: Required<CacheOptions>;

  // Statistics
  private hitCount = 0;
  private missCount = 0;
  private evictionCount = 0;

  constructor(options: CacheOptions = {}) {
    super();
    this.options = {
      maxSize: options.maxSize ?? 100 * 1024 * 1024, // 100MB default
      maxEntries: options.maxEntries ?? 10000,
      defaultTtl: options.defaultTtl ?? 0, // 0 = no expiry
      enableStatistics: options.enableStatistics ?? true,
      evictionPolicy: options.evictionPolicy ?? 'lru',
    };

    // Initialize scope index
    for (const scope of Object.values(CacheScope)) {
      this.scopeIndex.set(scope, new Set());
    }
  }

  // ==========================================================================
  // Core Cache Operations (6 functions)
  // ==========================================================================

  /**
   * Store a value in the cache
   */
  set<T>(key: string, value: T, scope: CacheScope = CacheScope.GLOBAL, ttl?: number): void {
    // Check if we need to evict
    this.ensureCapacity();

    const entry: CacheEntry<T> = {
      key,
      value,
      timestamp: Date.now(),
      ttl: ttl ?? this.options.defaultTtl,
      size: this.estimateSize(value),
      hits: 0,
    };

    // Remove from old scope if exists
    if (this.cache.has(key)) {
      this.removeFromScopeIndex(key);
    }

    this.cache.set(key, entry as CacheEntry);
    this.scopeIndex.get(scope)?.add(key);

    this.emit('cache-set', {
      key,
      scope,
      operation: 'set',
    } as CacheEventData);
  }

  /**
   * Retrieve a value from the cache
   */
  get<T>(key: string): T | undefined {
    const entry = this.cache.get(key) as CacheEntry<T> | undefined;

    if (!entry) {
      if (this.options.enableStatistics) {
        this.missCount++;
      }
      this.emit('cache-miss', { key, operation: 'get', hit: false } as CacheEventData);
      return undefined;
    }

    // Check TTL
    if (entry.ttl && entry.ttl > 0 && Date.now() - entry.timestamp > entry.ttl) {
      this.delete(key);
      if (this.options.enableStatistics) {
        this.missCount++;
      }
      this.emit('cache-expired', { key, operation: 'get', hit: false } as CacheEventData);
      return undefined;
    }

    // Update hit count for LFU
    const updatedEntry: CacheEntry<T> = {
      ...entry,
      hits: entry.hits + 1,
      timestamp: this.options.evictionPolicy === 'lru' ? Date.now() : entry.timestamp,
    };
    this.cache.set(key, updatedEntry as CacheEntry);

    if (this.options.enableStatistics) {
      this.hitCount++;
    }

    this.emit('cache-hit', { key, operation: 'get', hit: true } as CacheEventData);
    return entry.value;
  }

  /**
   * Check if a key exists in the cache
   */
  has(key: string): boolean {
    const entry = this.cache.get(key);
    if (!entry) return false;

    // Check TTL
    if (entry.ttl && entry.ttl > 0 && Date.now() - entry.timestamp > entry.ttl) {
      this.delete(key);
      return false;
    }

    return true;
  }

  /**
   * Remove a key from the cache
   */
  delete(key: string): boolean {
    const exists = this.cache.has(key);
    if (exists) {
      this.removeFromScopeIndex(key);
      this.cache.delete(key);
      this.emit('cache-delete', { key, operation: 'delete' } as CacheEventData);
    }
    return exists;
  }

  /**
   * Clear all entries from the cache
   */
  clear(): void {
    const count = this.cache.size;
    this.cache.clear();
    for (const scope of this.scopeIndex.values()) {
      scope.clear();
    }
    this.hitCount = 0;
    this.missCount = 0;
    this.evictionCount = 0;
    this.emit('cache-clear', { key: '*', operation: 'clear' } as CacheEventData);
  }

  /**
   * Clear entries by scope
   */
  clearScope(scope: CacheScope): number {
    const keys = this.scopeIndex.get(scope);
    if (!keys) return 0;

    const count = keys.size;
    for (const key of keys) {
      this.cache.delete(key);
    }
    keys.clear();

    this.emit('cache-scope-clear', { key: scope, scope, operation: 'clear' } as CacheEventData);
    return count;
  }

  // ==========================================================================
  // Statistics and Maintenance (4 functions)
  // ==========================================================================

  /**
   * Get cache statistics
   */
  getStatistics(): CacheStatistics {
    const entries = Array.from(this.cache.values());
    const totalSize = entries.reduce((sum, e) => sum + (e.size ?? 0), 0);
    const timestamps = entries.map((e) => e.timestamp);

    return {
      totalEntries: this.cache.size,
      totalSize,
      hitCount: this.hitCount,
      missCount: this.missCount,
      hitRate:
        this.hitCount + this.missCount > 0 ? this.hitCount / (this.hitCount + this.missCount) : 0,
      evictionCount: this.evictionCount,
      oldestEntry: timestamps.length > 0 ? Math.min(...timestamps) : undefined,
      newestEntry: timestamps.length > 0 ? Math.max(...timestamps) : undefined,
    };
  }

  /**
   * Set TTL for an existing entry
   */
  setTtl(key: string, ttl: number): boolean {
    const entry = this.cache.get(key);
    if (!entry) return false;

    const updatedEntry: CacheEntry = {
      ...entry,
      ttl,
    };
    this.cache.set(key, updatedEntry);
    return true;
  }

  /**
   * Get all cache keys
   */
  getKeys(scope?: CacheScope): string[] {
    if (scope) {
      return Array.from(this.scopeIndex.get(scope) ?? []);
    }
    return Array.from(this.cache.keys());
  }

  /**
   * Remove expired entries
   */
  prune(): number {
    const now = Date.now();
    let pruned = 0;

    for (const [key, entry] of this.cache.entries()) {
      if (entry.ttl && entry.ttl > 0 && now - entry.timestamp > entry.ttl) {
        this.delete(key);
        pruned++;
      }
    }

    if (pruned > 0) {
      this.emit('cache-prune', { key: '*', operation: 'delete' } as CacheEventData);
    }

    return pruned;
  }

  // ==========================================================================
  // Helper Methods
  // ==========================================================================

  /**
   * Ensure cache has capacity, evicting if needed
   */
  private ensureCapacity(): void {
    // Check entry count
    while (this.cache.size >= this.options.maxEntries) {
      this.evictOne();
    }

    // Check total size
    let totalSize = 0;
    for (const entry of this.cache.values()) {
      totalSize += entry.size ?? 0;
    }

    while (totalSize > this.options.maxSize && this.cache.size > 0) {
      const evicted = this.evictOne();
      totalSize -= evicted?.size ?? 0;
    }
  }

  /**
   * Evict one entry based on eviction policy
   */
  private evictOne(): CacheEntry | undefined {
    if (this.cache.size === 0) return undefined;

    let keyToEvict: string | undefined;

    switch (this.options.evictionPolicy) {
      case 'lru':
        // Find oldest timestamp
        keyToEvict = this.findOldestKey();
        break;

      case 'lfu':
        // Find least frequently used
        keyToEvict = this.findLeastUsedKey();
        break;

      case 'fifo':
      default:
        // First key
        keyToEvict = this.cache.keys().next().value;
        break;
    }

    if (keyToEvict) {
      const entry = this.cache.get(keyToEvict);
      this.removeFromScopeIndex(keyToEvict);
      this.cache.delete(keyToEvict);
      this.evictionCount++;
      this.emit('cache-evict', { key: keyToEvict, operation: 'evict' } as CacheEventData);
      return entry;
    }

    return undefined;
  }

  /**
   * Find key with oldest timestamp (LRU)
   */
  private findOldestKey(): string | undefined {
    let oldestKey: string | undefined;
    let oldestTime = Infinity;

    for (const [key, entry] of this.cache.entries()) {
      if (entry.timestamp < oldestTime) {
        oldestTime = entry.timestamp;
        oldestKey = key;
      }
    }

    return oldestKey;
  }

  /**
   * Find key with least hits (LFU)
   */
  private findLeastUsedKey(): string | undefined {
    let leastUsedKey: string | undefined;
    let leastHits = Infinity;

    for (const [key, entry] of this.cache.entries()) {
      if (entry.hits < leastHits) {
        leastHits = entry.hits;
        leastUsedKey = key;
      }
    }

    return leastUsedKey;
  }

  /**
   * Remove key from scope index
   */
  private removeFromScopeIndex(key: string): void {
    for (const keys of this.scopeIndex.values()) {
      keys.delete(key);
    }
  }

  /**
   * Estimate size of a value in bytes
   */
  private estimateSize(value: unknown): number {
    if (value === null || value === undefined) return 0;
    if (typeof value === 'string') return value.length * 2;
    if (typeof value === 'number') return 8;
    if (typeof value === 'boolean') return 4;
    if (Buffer.isBuffer(value)) return value.length;
    if (Array.isArray(value)) {
      return value.reduce((sum, v) => sum + this.estimateSize(v), 0);
    }
    if (typeof value === 'object') {
      return JSON.stringify(value).length * 2;
    }
    return 0;
  }

  /**
   * Cleanup resources
   */
  destroy(): void {
    this.clear();
    this.removeAllListeners();
  }
}

export default CacheManager;
