/**
 * Worker Threads Module
 * Exports worker pool and types for parallel PDF processing
 */

export type { WorkerResult, WorkerTask } from './pool.js';
export { WorkerPool, workerPool } from './pool.js';
