"""Simple test support functions for modal-rs function call testing."""

import modal

app = modal.App("libmodal-rs-test-support")


@app.function()
def echo_string(s: str) -> str:
    return f"echo: {s}"


@app.function()
def add_numbers(a: int, b: int) -> int:
    return a + b
