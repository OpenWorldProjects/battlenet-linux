"""Exercise the compiled Rust PE bridge in a disposable Wine prefix.

Only synthetic tickets are used. Existing prefixes and accounts are untouched.
Run: xvfb-run -a python3 scripts/test-bridge.py
"""
import os
import shutil
import socket
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
helper = root / "assets/bnet-session-bridge.exe"
fixture = root / "bridge/target/x86_64-pc-windows-msvc/release/examples/launch_fixture.exe"
if not fixture.is_file():
    subprocess.run(["sh", str(root / "scripts/build-bridge.sh"), "--tests"], check=True)
with tempfile.TemporaryDirectory(prefix="bnet-bridge-test-") as temporary:
    env = {**os.environ, "WINEPREFIX": str(Path(temporary) / "prefix"), "WINEDEBUG": "-all"}

    def exchange(frame, code, response):
        result = subprocess.run(["wine", str(helper)], input=frame.encode(),
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                env=env, timeout=60)
        assert result.returncode == code, ("unexpected exit", result.returncode, result.stderr.decode(errors="replace"))
        assert result.stdout == response.encode(), ("unexpected acknowledgement", result.stdout)

    def exchange_network(frame, expected):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(2)
            listener.settimeout(45)
            nonce = "12" * 32
            network_env = {**env, "BNET_BRIDGE_PORT": str(listener.getsockname()[1]), "BNET_BRIDGE_NONCE": nonce}
            process = subprocess.Popen(["wine", str(helper)], stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=network_env)
            try:
                stream, _ = listener.accept()
                with stream:
                    stream.settimeout(10)
                    hello = b""
                    while len(hello) < 64:
                        part = stream.recv(64 - len(hello))
                        assert part, "Missing bridge nonce"
                        hello += part
                    assert hello == nonce.encode()
                    stream.sendall(frame.encode())
                    stream.shutdown(socket.SHUT_WR)
                    reply = b""
                    while part := stream.recv(256):
                        reply += part
                    assert reply == expected.encode(), ("unexpected bridge reply", reply)
                assert process.wait(timeout=10) == (4 if expected == "exit:4\n" else 87)
            finally:
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=10)

    server = shutil.which("wineserver") or shutil.which("wineserver64") or next((str(p) for p in [Path("/usr/lib/x86_64-linux-gnu/wine/wineserver"), Path("/usr/lib/wine/wineserver")] if p.exists()), None)
    try:
        exchange("BNET-SC2-1\nprobe\n", 4, "")
        exchange("BNET-SC2-1\nwrite\ninvalid\nEU\nenUS\n\n", 2, "")
        # Not a real account or credential.
        token = "EU-" + "0" * 32 + "-12345"
        exchange(f"BNET-SC2-1\nwrite\n{token}\nEU\nenUS\n\n", 87, "stored\n")
        exchange("BNET-SC2-1\nprobe\n", 87, "ticket-ready\n")
        exchange("BNET-SC2-1\nprobe-account\n12345\nEU\nenUS\n", 87, "ticket-ready\n")
        exchange("BNET-SC2-1\nprobe-account\n99999\nEU\nenUS\n", 4, "")
        exchange("BNET-SC2-1\nprobe-account\n12345\nUS\nenUS\n", 87, "ticket-ready\n")
        exchange("BNET-SC2-1\nprobe-account\n12345\nEU\nenUS\nextra\n", 2, "")
        game_dir = Path(temporary) / "SC2 test install"
        (game_dir / "Support64").mkdir(parents=True)
        (game_dir / "Versions/Base12345").mkdir(parents=True)
        shutil.copyfile(fixture, game_dir / "Versions/Base12345/SC2_x64.exe")
        shutil.copyfile(fixture, game_dir / "Support64/SC2Switcher_x64.exe")
        windows_root = "Z:" + str(game_dir).replace("/", "\\")
        # The fixture returns 47 only when cwd and quoted -dataDir agree.
        exchange(f"BNET-SC2-1\nresume\n{windows_root}\nBase12345\n", 47, "started\n")
        exchange(f"BNET-SC2-1\nresume\n{windows_root}\n\n", 47, "started\n")
        exchange("BNET-SC2-1\nresume\nZ:\\missing-install\n", 6, "")
        exchange("BNET-SC2-1\nresume\nZ:\\missing-install\nBase12345\n", 6, "")
        exchange("BNET-SC2-1\nresume\nZ:\\missing-install\nBase../bad\n", 2, "")
        exchange("BNET-SC2-1\nclear\n", 87, "cleared\n")
        exchange("BNET-SC2-1\nprobe\n", 4, "")
        exchange("BNET-SC2-1\nresume\nZ:\\missing-install\n", 4, "")
        exchange("BNET-SC2-1\nclear\n", 87, "cleared\n")
        exchange("BNET-SC2-1\nclear\nextra\n", 2, "")
        exchange_network(f"BNET-SC2-1\nwrite\n{token}\nEU\nenUS\n\n", "stored\nexit:87\n")
        exchange_network("BNET-SC2-1\nprobe\n", "ticket-ready\nexit:87\n")
        exchange_network("BNET-SC2-1\nprobe-account\n12345\nEU\nenUS\n", "ticket-ready\nexit:87\n")
        exchange_network("BNET-SC2-1\nprobe-account\n99999\nEU\nenUS\n", "exit:4\n")
        exchange_network("BNET-SC2-1\nclear\n", "cleared\nexit:87\n")
        print("PASS: correct game cwd/dataDir, propagated exit 47, DPAPI, launch gate, and loopback transport")
    finally:
        if server:
            subprocess.run([server, "-k"], env=env, timeout=10, check=False)
            subprocess.run([server, "-w"], env=env, timeout=10, check=False)
