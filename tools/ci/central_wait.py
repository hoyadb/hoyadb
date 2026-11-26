"""Wait for Central PUBLISHED; upload/validation alone does not unblock npm."""
import argparse
import json
import os
from pathlib import Path
import re
import time
from urllib.request import Request, urlopen


def published(response):
    state = response.get("deploymentState")
    if state == "PUBLISHED":
        return True
    if state in ("PENDING", "VALIDATING", "VALIDATED", "PUBLISHING"):
        return False
    raise ValueError("Central deployment failed or returned an unknown state")


def fetch(identifier, token):
    request = Request("https://central.sonatype.com/api/v1/publisher/status?id=" + identifier,
                      headers={"Authorization": "Bearer " + token}, method="POST")
    with urlopen(request, timeout=30) as response:
        return json.load(response)


def wait(identifier, token, attempts=60):
    if not re.fullmatch(r"[0-9a-f-]{36}", identifier) or not token:
        raise ValueError("Central deployment identity or token is missing")
    for _ in range(attempts):
        if published(fetch(identifier, token)):
            print("Central deployment PUBLISHED")
            return
        time.sleep(10)
    raise TimeoutError("Central has not confirmed publication; downstream publishing stopped")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("deployment_file", type=Path)
    args = parser.parse_args()
    wait(args.deployment_file.read_text().strip(), os.environ.get("CENTRAL_TOKEN", ""))
