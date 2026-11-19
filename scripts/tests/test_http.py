"""Fixture HTTP policy controls; no network calls or real credentials."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
from urllib.request import Request, ProxyHandler

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import http


class LocalHTTPTests(unittest.TestCase):
    @patch.object(http, 'build_opener')
    def test_rejects_nonlocal_or_credential_bearing_urls_before_network(self, build):
        for url in ('http://example.com:80/', 'http://localhost:80/',
                    'https://127.0.0.1:80/', 'http://127.0.0.1/',
                    'http://name:secret@127.0.0.1:80/', 'http://127.0.0.1:80/#fragment'):
            with self.subTest(url=url), self.assertRaises(ValueError):
                http.open_local(url)
        build.assert_not_called()

    @patch.object(http, 'build_opener')
    def test_local_bearer_never_uses_proxy_or_redirect_handlers(self, build):
        request = Request('http://127.0.0.1:3975/v2/users',
                          headers={'Authorization': 'Bearer synthetic'})
        http.open_local(request, timeout=4)
        proxy, redirect = build.call_args.args
        self.assertIsInstance(proxy, ProxyHandler)
        self.assertEqual(proxy.proxies, {})
        self.assertIsNone(redirect.redirect_request(request, None, 302, '', {},
                                                   'http://example.com/collect'))
        build.return_value.open.assert_called_once_with(request, timeout=4)


if __name__ == '__main__':
    unittest.main()
