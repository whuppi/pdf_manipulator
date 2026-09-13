/**
 * Worker Thread Script
 * Handles off-main-thread PDF processing tasks
 */

import { parentPort } from 'worker_threads';
import type { WorkerResult, WorkerTask } from './pool.js';

// Types for operations - will be available at runtime via the PdfDocument
type PdfDocument = any;

/**
 * Process a worker task
 */
async function handleTask(task: WorkerTask<any>): Promise<WorkerResult<any>> {
  const startTime = Date.now();

  try {
    // Dynamically import PdfDocument since we can't use top-level imports
    // in a worker context reliably across all environments
    const { PdfDocument } = await import('../index.js');

    if (!PdfDocument) {
      throw new Error('PdfDocument not available in worker');
    }

    let result: any;

    switch (task.operation) {
      case 'extract': {
        const doc = PdfDocument.open(task.documentPath);
        const extMgr = doc.extraction;

        if (task.params.type === 'markdown') {
          result = extMgr.extractMarkdown(task.params.pageIndex, task.params.options);
        } else if (task.params.type === 'html') {
          result = extMgr.extractHtml(task.params.pageIndex, task.params.options);
        } else {
          result = extMgr.extractText(task.params.pageIndex, task.params.options);
        }
        break;
      }

      case 'search': {
        const doc = PdfDocument.open(task.documentPath);
        const searchMgr = doc.search;
        result = searchMgr.searchAll(task.params.query, task.params.options || {});
        break;
      }

      case 'render': {
        const doc = PdfDocument.open(task.documentPath);
        const renderMgr = doc.rendering;
        result = renderMgr.renderPage(task.params.pageIndex, task.params.options || {});
        break;
      }

      case 'analyze': {
        const doc = PdfDocument.open(task.documentPath);

        result = {
          pageCount: doc.pageCount,
          metadata: doc.metadata?.getMetadata?.() || null,
          outline: {
            count: doc.outline?.getOutlineCount?.() || 0,
            isFlat: doc.outline?.isFlat?.() || false,
          },
          layers: {
            count: doc.layers?.getLayerCount?.() || 0,
            visible: doc.layers?.getVisibleLayerCount?.() || 0,
          },
        };
        break;
      }

      default:
        throw new Error(`Unknown operation: ${task.operation}`);
    }

    const duration = Date.now() - startTime;

    return {
      success: true,
      data: result,
      duration,
    };
  } catch (error) {
    const duration = Date.now() - startTime;

    return {
      success: false,
      error:
        error instanceof Error
          ? {
              name: error.name,
              message: error.message,
              stack: error.stack,
            }
          : String(error),
      duration,
    };
  }
}

/**
 * Main worker message handler
 */
if (parentPort) {
  parentPort.on('message', async (task: WorkerTask<any>) => {
    const result = await handleTask(task);
    parentPort?.postMessage(result);
  });
} else {
  console.error('Worker script must be run as a Worker thread');
  process.exit(1);
}
