"""Loopback HTTP transport for fixtures; never forward local bearers through proxies or redirects."""
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, ProxyHandler, Request, build_opener


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        """Returning None makes urllib reject the redirect rather than copying Authorization."""
        return None


def open_local(request, *, timeout=30):
    """Validate the initial URL as well as disabling all subsequent redirect/proxy routing."""
    url = request.full_url if isinstance(request, Request) else request
    parsed = urlsplit(url)
    if (parsed.scheme != 'http' or parsed.hostname != '127.0.0.1' or parsed.port is None
            or parsed.username is not None or parsed.password is not None or parsed.fragment):
        raise ValueError('Development HTTP transport requires numeric loopback without credentials')
    return build_opener(ProxyHandler({}), _NoRedirect()).open(request, timeout=timeout)
