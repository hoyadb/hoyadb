"""TC-0036 application-owned loopback transport fixture, not SDK networking."""
import argparse
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


def outcomes(values, mode='applied'):
    if not isinstance(values, list) or mode not in ('applied', 'rejected'):
        raise ValueError()
    result = []
    for envelope in values:
        if not isinstance(envelope, dict):
            raise ValueError()
        if 'writeId' in envelope:
            correlation = {key: envelope[key] for key in ('table', 'recordId', 'writeId')}
            business = {'id': envelope['recordId'], 'created': envelope['op'] == 'create', 'echo': envelope['changes']}
        else:
            correlation = {'idempotencyKey': envelope['idempotencyKey']}
            business = envelope['args']
        if not all(isinstance(value, str) for value in correlation.values()):
            raise ValueError()
        outcome = {'status': 'applied', 'result': business} if mode == 'applied' else {'status': 'rejected', 'reason': 'application fixture rejection'}
        result.append({**correlation, **outcome})
    return result[::-1]


class Handler(SimpleHTTPRequestHandler):
    def do_POST(self):
        if self.path != '/hoyadb/commands':
            self.send_error(404)
            return
        try:
            size = int(self.headers.get('Content-Length', '-1'))
            if not 0 <= size <= 1048576:
                raise ValueError()
            values = json.loads(self.rfile.read(size).decode('utf-8'))
            result = outcomes(values, self.headers.get('X-Hoya-Mode', 'applied'))
            payload = json.dumps(result, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
        except (ValueError, KeyError, TypeError):
            self.send_error(400)
            return
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args):
        pass  # Fixture bodies/context/keys never enter automatic logs.


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8878)
    args = parser.parse_args()
    ThreadingHTTPServer(('127.0.0.1', args.port), Handler).serve_forever()
