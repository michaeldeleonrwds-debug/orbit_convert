import json
import unittest
from orbit_engine.protocol import CommandRequest, CommandResponse


class TestProtocol(unittest.TestCase):
    def test_request_serialization(self):
        req = CommandRequest(
            command="quick_action",
            job_id="test_job_123",
            input_file="C:/test/sample.jpg",
            output_file="C:/test/sample.png",
            action="remove_background",
            options={"alpha_matting": True},
        )
        as_json = json.dumps(req.__dict__)
        parsed = CommandRequest.from_json(as_json)
        self.assertEqual(parsed.command, "quick_action")
        self.assertEqual(parsed.job_id, "test_job_123")
        self.assertEqual(parsed.action, "remove_background")
        self.assertTrue(parsed.options.get("alpha_matting"))

    def test_response_serialization(self):
        resp = CommandResponse(
            job_id="test_job_123",
            operation="remove_background",
            status="success",
            progress=1.0,
            processing_time_ms=124.5,
        )
        raw = resp.to_dict()
        self.assertEqual(raw["status"], "success")
        self.assertEqual(raw["progress"], 1.0)
        self.assertNotIn("error_code", raw)


if __name__ == "__main__":
    unittest.main()
