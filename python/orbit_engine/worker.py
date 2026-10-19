"""Persistent worker daemon executing conversion and AI processing requests."""

from __future__ import annotations
import os
import sys
import time
import traceback
from typing import Dict, Set

from .background_removal import get_ai_remover
from .dependencies import get_full_dependency_status
from .protocol import CommandRequest, CommandResponse, log_diag
from .registry import get_engine_capabilities

# Active cancelled job IDs
_cancelled_jobs: Set[str] = set()


def handle_request(req: CommandRequest) -> None:
    job_id = req.job_id or "job_init"
    cmd = req.command.strip().lower()
    start_time = time.perf_counter()

    log_diag(f"Received command: '{cmd}', job_id: '{job_id}'")

    if cmd == "shutdown":
        CommandResponse(
            job_id=job_id,
            operation="shutdown",
            status="success",
        ).send()
        log_diag("Shutdown command acknowledged. Exiting.")
        sys.exit(0)

    elif cmd == "cancel":
        target_job = req.options.get("target_job_id", job_id)
        _cancelled_jobs.add(target_job)
        CommandResponse(
            job_id=job_id,
            operation="cancel",
            status="success",
            payload={"cancelled_job_id": target_job},
        ).send()
        return

    elif cmd == "capabilities":
        caps = get_engine_capabilities()
        elapsed_ms = (time.perf_counter() - start_time) * 1000.0
        CommandResponse(
            job_id=job_id,
            operation="capabilities",
            status="success",
            processing_time_ms=round(elapsed_ms, 2),
            payload=caps,
        ).send()
        return

    elif cmd == "dependency_status":
        deps = get_full_dependency_status()
        elapsed_ms = (time.perf_counter() - start_time) * 1000.0
        CommandResponse(
            job_id=job_id,
            operation="dependency_status",
            status="success",
            processing_time_ms=round(elapsed_ms, 2),
            payload=deps,
        ).send()
        return

    elif cmd == "quick_action":
        action = (req.action or req.options.get("action", "")).lower()

        if action in ("remove_background", "removebackground"):
            input_file = req.input_file
            output_file = req.output_file

            if not input_file or not output_file:
                CommandResponse(
                    job_id=job_id,
                    operation=action,
                    status="error",
                    error_code="INVALID_ARGUMENTS",
                    error_message="Both input_file and output_file are required for remove_background",
                ).send()
                return

            def on_progress(p: float, stage: str) -> None:
                if job_id in _cancelled_jobs:
                    raise RuntimeError("OPERATION_CANCELLED")
                CommandResponse(
                    job_id=job_id,
                    operation=action,
                    status="progress",
                    input_file=input_file,
                    output_file=output_file,
                    progress=p,
                    stage=stage,
                ).send()

            try:
                # Initial progress
                on_progress(0.05, "Starting AI background removal")

                remover = get_ai_remover()
                res = remover.remove_background(
                    input_path=input_file,
                    output_path=output_file,
                    model_name=req.options.get("model"),
                    alpha_matting=req.options.get("alpha_matting", True),
                    alpha_matting_foreground_threshold=req.options.get("foreground_threshold", 240),
                    alpha_matting_background_threshold=req.options.get("background_threshold", 10),
                    alpha_matting_erode_size=req.options.get("erode_size", 10),
                    progress_callback=on_progress,
                )

                elapsed_ms = (time.perf_counter() - start_time) * 1000.0
                CommandResponse(
                    job_id=job_id,
                    operation=action,
                    status="success",
                    input_file=input_file,
                    output_file=output_file,
                    progress=1.0,
                    stage="Complete",
                    processing_time_ms=round(elapsed_ms, 2),
                    payload=res,
                ).send()
            except Exception as e:
                err_msg = str(e)
                if "OPERATION_CANCELLED" in err_msg or job_id in _cancelled_jobs:
                    _cancelled_jobs.discard(job_id)
                    CommandResponse(
                        job_id=job_id,
                        operation=action,
                        status="cancelled",
                        input_file=input_file,
                        output_file=output_file,
                    ).send()
                else:
                    log_diag(f"Background removal error: {traceback.format_exc()}")
                    CommandResponse(
                        job_id=job_id,
                        operation=action,
                        status="error",
                        input_file=input_file,
                        output_file=output_file,
                        error_code="PROCESSING_FAILED",
                        error_message=err_msg,
                    ).send()
            finally:
                _cancelled_jobs.discard(job_id)
            return

        else:
            CommandResponse(
                job_id=job_id,
                operation=action,
                status="error",
                error_code="UNSUPPORTED_ACTION",
                error_message=f"Quick action '{action}' is not supported by Python worker",
            ).send()
            return

    else:
        CommandResponse(
            job_id=job_id,
            operation=cmd,
            status="error",
            error_code="UNKNOWN_COMMAND",
            error_message=f"Command '{cmd}' is not recognized",
        ).send()


def main() -> None:
    log_diag(f"OrbitConvert Python Worker daemon started on PID {os.getpid()}")
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            req = CommandRequest.from_json(line)
            handle_request(req)
        except Exception as e:
            log_diag(f"Malformed input request: {line} -> {e}")
            CommandResponse(
                job_id="malformed_request",
                operation="parse",
                status="error",
                error_code="MALFORMED_JSON",
                error_message=str(e),
            ).send()


if __name__ == "__main__":
    main()
