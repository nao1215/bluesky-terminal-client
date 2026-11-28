"""Run the real fixture with reverse DNS unavailable, including media startup."""
from unittest.mock import patch

import relay


if __name__ == "__main__":
    # A resolver stall must not prevent the loopback fixture from becoming ready.
    with patch("socket.getfqdn", side_effect=AssertionError("fixture must not use DNS")):
        relay.main()
