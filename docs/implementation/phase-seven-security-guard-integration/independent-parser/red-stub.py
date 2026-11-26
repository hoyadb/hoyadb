"""Reader-format property parsing for SPEC-0034 / TC-0038."""


class MalformedProperties(Exception):
    """A Unicode escape is incomplete or contains a non-hexadecimal digit."""


def load_reader(text: str) -> dict[str, str]:
    """Parse an already decoded Properties Reader character stream."""
    return {}
