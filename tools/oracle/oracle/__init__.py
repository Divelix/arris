"""Open CASCADE oracle for the Arris fixture corpus.

`recipe` builds a shape from a fixture's recipe, `measure` turns a shape into
the numbers `expected.json` holds. Both raise `OracleError` with a message
that names the step or quantity; nothing here is silent.
"""

import importlib.metadata


class OracleError(Exception):
    """A recipe that cannot be built or a shape that cannot be measured."""


def occt_version() -> str:
    """The pinned wheel's version, recorded in every expected.json."""
    return importlib.metadata.version("cadquery-ocp")


def require_ocp() -> None:
    """Fail loudly, with the fix, when OCP is not importable.

    The scripts call this before anything else, so running them outside the
    project's environment prints one clear line instead of a traceback.
    """
    try:
        import OCP  # noqa: F401
    except ImportError as e:  # pragma: no cover - only reachable outside the venv
        raise SystemExit(
            "oracle: Open CASCADE (OCP) is not importable — run through the "
            "project's environment: `uv run --project tools/oracle <script>` "
            "(tools/oracle/README.md). Cause: " + str(e)
        ) from e
