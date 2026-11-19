"""Loopback-only demo viewer with preconfigured fixture identities, not production sign-in."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs
from urllib.error import HTTPError
import support as s


def serve():
    """Keep signing keys server-side and proxy only the two fixed read-only API operations."""
    class Handler(BaseHTTPRequestHandler):
        """Serve only the viewer and fixed fixture queries; never expose signing keys."""
        def log_message(self, *_):
            """Suppress request logging because URLs and responses can identify private contracts."""
            pass  # Never log tokens, query strings or indexed contract data.

        def do_GET(self):
            """Validate the local host and fixture identity before proxying authorized reads."""
            route = urlparse(self.path)
            host = self.headers.get('Host', '')
            # Reject other Host names to keep a browser's DNS alias from reaching
            # this fixture-identity proxy through a rebound domain.
            if host not in ('127.0.0.1:18891', 'localhost:18891'):
                self.send_error(403); return
            try:
                if route.path == '/':
                    body = (s.ROOT / 'examples/poc/dashboard.html').read_bytes(); kind = 'text/html; charset=utf-8'
                elif route.path == '/data':
                    subject = parse_qs(route.query).get('user', ['a'])[0]
                    if subject not in ('a', 'b', 'ab'):
                        self.send_error(400); return
                    # Both calls use the selected fixture principal; its bearer is
                    # minted server-side and never returned to browser JavaScript.
                    body = json.dumps({'model': s.query(subject), 'history': s.query(subject, typed=False)}).encode()
                    kind = 'application/json'
                else:
                    self.send_error(404); return
                self.send_response(200)
                self.send_header('Content-Type', kind)
                self.send_header('Cache-Control', 'no-store')
                self.send_header('X-Content-Type-Options', 'nosniff')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers(); self.wfile.write(body)
            except (HTTPError, OSError, ValueError):
                self.send_error(503, 'Indexer API unavailable')
    print('Demo viewer: http://127.0.0.1:18891 — fixture identities only; Ctrl+C stops owned child processes')
    server = ThreadingHTTPServer(('127.0.0.1', 18891), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
