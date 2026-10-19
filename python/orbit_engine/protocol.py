"""Versioned JSON protocol for Rust <-> Python IPC communication."""

from __future__ import annotations
import json
import sys
import time
from dataclasses import dataclass, field, asdict
from typing import Any, Dict, List, Optional

PROTOCOL_VERSION = "1.0"


@dataclass
class CommandRequest:
    command: str
    job_id: str
    input_file: Optional[str] = None
    output_file: Optional[str] = None
    action: Optional[str] = None
    options: Dict[str, Any] = field(default_factory=dict)
    protocol_version: str = PROTOCOL_VERSION

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> CommandRequest:
        return cls(
            command=data.get("command", ""),
            job_id=data.get("job_id", ""),
            input_file=data.get("input_file"),
            output_file=data.get("output_file"),
            action=data.get("action"),
            options=data.get("options", {}),
            protocol_version=data.get("protocol_version", PROTOCOL_VERSION),
        )

    @classmethod
    def from_json(cls, json_str: str) -> CommandRequest:
        return cls.from_dict(json.loads(json_str))


@dataclass
class CommandResponse:
    job_id: str
    operation: str
    status: str  # "started", "progress", "success", "error", "cancelled"
    input_file: Optional[str] = None
    output_file: Optional[str] = None
    progress: Optional[float] = None
    stage: Optional[str] = None
    error_code: Optional[str] = None
    error_message: Optional[str] = None
    warnings: List[str] = field(default_factory=list)
    processing_time_ms: Optional[float] = None
    payload: Optional[Dict[str, Any]] = None
    protocol_version: str = PROTOCOL_VERSION

    def to_dict(self) -> Dict[str, Any]:
        d = asdict(self)
        # Prune None keys where appropriate for clean JSON
        return {k: v for k, v in d.items() if v is not None}

    def to_json(self) -> str:
        return json.dumps(self.to_dict())

    def send(self) -> None:
        """Write formatted response JSON to stdout as a single line, then flush."""
        line = self.to_json()
        sys.stdout.write(line + "\n")
        sys.stdout.flush()


def log_diag(msg: str) -> None:
    """Diagnostic logs are sent strictly to stderr, never polluting the protocol stdout stream."""
    sys.stderr.write(f"[OrbitEngine] {msg}\n")
    sys.stderr.flush()
