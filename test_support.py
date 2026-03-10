"""Simple test support functions for modal-rs function call testing."""

import os

import modal

app = modal.App("libmodal-rs-test-support")


@app.function()
def echo_string(s: str) -> str:
    return f"echo: {s}"


@app.function()
def add_numbers(a: int, b: int) -> int:
    return a + b


@app.cls()
class EchoCls:
    @modal.method()
    def echo_string(self, s: str) -> str:
        return f"cls echo: {s}"

    @modal.method()
    def echo_env_var(self, key: str) -> str:
        return os.environ.get(key, f"<{key} not set>")
