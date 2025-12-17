/**
 * markdown-media
 *
 * The Universal Standard for integrating Rich Media (SVG, Images, Video) into Markdown.
 * Core engine for HWP/PDF conversion.
 *
 * @author beasthan2025
 * @license MIT
 * @version 0.0.1
 */

'use strict';

/**
 * Markdown-Media Bundle Format
 *
 * A bundle consists of:
 * - index.md: Pure text content in Markdown format
 * - media/: Directory containing SVG, PNG, JPG files
 * - meta.json: Metadata about the original document
 */

const VERSION = '0.0.1';

/**
 * Placeholder for the main conversion function
 * Will be implemented with Rust/Python bindings
 *
 * @param {string} inputPath - Path to input file (HWP, PDF, DOCX)
 * @param {Object} options - Conversion options
 * @returns {Promise<Object>} - Converted bundle info
 */
async function convert(inputPath, options = {}) {
  // TODO: Implement with Rust core engine
  throw new Error('Not implemented yet. Coming soon in v1.0.0');
}

/**
 * Placeholder for bundle validation
 *
 * @param {string} bundlePath - Path to markdown-media bundle
 * @returns {boolean} - Whether the bundle is valid
 */
function validateBundle(bundlePath) {
  // TODO: Implement bundle validation
  throw new Error('Not implemented yet. Coming soon in v1.0.0');
}

/**
 * Get supported input formats
 *
 * @returns {string[]} - List of supported file extensions
 */
function getSupportedFormats() {
  return ['.hwp', '.hwpx', '.pdf', '.docx', '.doc'];
}

/**
 * Get library version
 *
 * @returns {string} - Version string
 */
function getVersion() {
  return VERSION;
}

module.exports = {
  convert,
  validateBundle,
  getSupportedFormats,
  getVersion,
  VERSION
};
