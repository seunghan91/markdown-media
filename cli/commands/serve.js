import { createServer } from 'http';
import { readFile, stat } from 'fs/promises';
import { resolve, join, extname } from 'path';
import { existsSync } from 'fs';

export async function serveCommand(path, options) {
  const servePath = resolve(path);
  const port = parseInt(options.port);

  if (!existsSync(servePath)) {
    console.error(`❌ Error: Path not found: ${path}`);
    process.exit(1);
  }

  const server = createServer(async (req, res) => {
    try {
      let filePath = join(servePath, req.url === '/' ? 'index.html' : req.url);
      
      // Security: prevent directory traversal
      if (!filePath.startsWith(servePath)) {
        res.writeHead(403);
        res.end('Forbidden');
        return;
      }

      const stats = await stat(filePath);
      
      if (stats.isDirectory()) {
        filePath = join(filePath, 'index.html');
      }

      const content = await readFile(filePath);
      const ext = extname(filePath);
      
      // Set content type
      const contentTypes = {
        '.html': 'text/html',
        '.js': 'text/javascript',
        '.css': 'text/css',
        '.json': 'application/json',
        '.mdx': 'text/markdown',
        '.md': 'text/markdown',
        '.png': 'image/png',
        '.jpg': 'image/jpeg',
        '.svg': 'image/svg+xml',
      };
      
      res.writeHead(200, {
        'Content-Type': contentTypes[ext] || 'text/plain',
      });
      res.end(content);
    } catch (err) {
      if (err.code === 'ENOENT') {
        res.writeHead(404);
        res.end('Not Found');
      } else {
        res.writeHead(500);
        res.end('Server Error');
      }
    }
  });

  server.listen(port, () => {
    console.log(`🚀 MDM Preview Server`);
    console.log(`   Serving: ${servePath}`);
    console.log(`   URL: http://localhost:${port}`);
    console.log('\n   Press Ctrl+C to stop');
    
    if (options.open) {
      // Dynamic import is async, handle properly
      import('child_process').then(({ exec }) => {
        exec(`open http://localhost:${port}`);
      });
    }
  });
}
