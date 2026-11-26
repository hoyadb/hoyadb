"""TC-0054: downstream registries wait for actual Maven publication."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


class CentralWaitTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("central_wait", Path(__file__).with_name("central_wait.py"))
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)

    def test_publishing_is_not_success_and_failed_or_unknown_states_stop(self):
        self.assertTrue(self.module.published({"deploymentState": "PUBLISHED"}))
        for state in ("PENDING", "VALIDATING", "VALIDATED", "PUBLISHING"):
            self.assertFalse(self.module.published({"deploymentState": state}))
        for response in ({}, {"deploymentState": "FAILED"}, {"deploymentState": "UNKNOWN"}):
            with self.assertRaises(ValueError):
                self.module.published(response)

    def test_timeout_does_not_report_publication_success(self):
        with patch.object(self.module, "fetch", return_value={"deploymentState": "PUBLISHING"}), \
                patch.object(self.module.time, "sleep"):
            with self.assertRaises(TimeoutError):
                self.module.wait("28570f16-da32-4c14-bd2e-c1acc0782365", "fixture-token", attempts=2)


if __name__ == "__main__":
    unittest.main()
