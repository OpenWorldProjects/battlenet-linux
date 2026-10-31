"""Test callback interception without an account or Internet login."""
import os
from pathlib import Path
import subprocess
import json
import tempfile
import time

helper = Path(__file__).resolve().parent.parent / "login-helper/target/debug/bnet-login"
assert subprocess.check_output([str(helper),"--protocol-version"]) == b"bnet-login/2\n"
env = {**os.environ, "GDK_BACKEND": "x11"}
env.pop("WAYLAND_DISPLAY", None)
for name in ["GIO_MODULE_DIR", "GTK_PATH", "GTK_MODULES"]:
    env.pop(name, None)
result = subprocess.run([str(helper), "EU", "--test-callback"], env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
assert result.returncode == 0, (result.returncode, result.stderr.decode(errors="replace"))
assert result.stdout == b"EU-00000000000000000000000000000000-12345", "Unexpected synthetic callback result"
print("PASS: automatic WebKit callback capture through a private pipe")

# A genuine HTTP redirect exercises a different WebKit path than load_uri.
import http.server
import threading
class Redirect(http.server.BaseHTTPRequestHandler):
    def send_json(self, data, status=200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())
    def do_GET(self):
        if self.path == "/account-start":
            self.send_response(302)
            self.send_header("Location", "/account")
            self.send_header("Set-Cookie", "session=verified; Path=/; Max-Age=3600; HttpOnly; SameSite=Lax")
            self.end_headers()
            return
        if self.path == "/account":
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<html><title>Account fixture</title><body>Account settings</body></html>")
            return
        if self.path == "/api/details":
            self.send_json({"battleTag":"FixtureUser#1234", "email":"never-export@example.invalid"})
            return
        token_url = "bnet-linux-login://callback/?ST=EU-00000000000000000000000000000000-12345"
        pages = {
            "/script": f"<script>window.location.href={token_url!r}</script>",
            "/form": "<form method='post' action='/finish'></form><script>document.forms[0].submit()</script>",
            "/popup": "<script>window.open('/login')</script>",
        }
        if self.path in pages:
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(pages[self.path].encode())
        else:
            self.send_response(302)
            self.send_header("Location", token_url)
            self.end_headers()
    def do_POST(self):
        if self.path == "/api/":
            authenticated = "session=verified" in self.headers.get("Cookie", "")
            self.send_json({"authenticated":authenticated,"accountId":"12345"}, 200 if authenticated else 401)
            return
        self.send_response(303)
        self.send_header("Location", "bnet-linux-login://callback/?ST=EU-00000000000000000000000000000000-12345")
        self.end_headers()
    def log_message(self, *_):
        pass
with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Redirect) as server:
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for route in ["login", "script", "form", "popup"]:
            redirect_env = {**env, "BNET_TEST_LOGIN_URL": f"http://127.0.0.1:{server.server_port}/{route}"}
            result = subprocess.run([str(helper), "EU", "--test-redirect"], env=redirect_env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
            assert result.returncode == 0, (route, result.returncode, result.stderr.decode(errors="replace"))
            assert result.stdout == b"EU-00000000000000000000000000000000-12345", route
            print(f"PASS: automatic {route} return without copying a URL")
        with tempfile.TemporaryDirectory(prefix="bnet-account-test-") as directory:
            account_env={**env,"BNET_TEST_LOGIN_URL":f"http://127.0.0.1:{server.server_port}/account-start"}
            result=subprocess.run([str(helper),"EU","--test-account","--session-dir",directory],env=account_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
            assert result.returncode==0,(result.returncode,result.stderr.decode(errors="replace"))
            assert json.loads(result.stdout)=={"account_id":"12345","battle_tag":"FixtureUser#1234"}
            assert b"never-export" not in result.stdout
            print("PASS: account verified independently of a game; only account ID and BattleTag returned")
            account_env["BNET_TEST_LOGIN_URL"]=f"http://127.0.0.1:{server.server_port}/account"
            result=subprocess.run([str(helper),"EU","--test-account","--session-dir",directory],env=account_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
            assert result.returncode==0,(result.returncode,result.stderr.decode(errors="replace"))
            assert json.loads(result.stdout)["account_id"]=="12345"
            print("PASS: private browser session reused after closing the login helper")
        denied_env={**env,"BNET_TEST_LOGIN_URL":f"http://127.0.0.1:{server.server_port}/account"}
        process=subprocess.Popen([str(helper),"EU","--test-account"],env=denied_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        try:
            time.sleep(2)
            assert process.poll() is None, "Unauthenticated account was accepted"
        finally:
            process.terminate()
            output,_=process.communicate(timeout=10)
        assert output==b"", "Unauthenticated account produced a profile"
        print("PASS: unsigned account cannot unlock the launcher")
    finally:
        server.shutdown()
