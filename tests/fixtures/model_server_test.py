#!/usr/bin/env python3
"""The virtual model (model_server.py): its scripts, answers and log, over HTTP as Claude Code and Echo call it."""
import importlib.util
import json
import os
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("model_server", os.path.join(os.path.dirname(os.path.abspath(__file__)), "model_server.py"))
ms = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ms)

SCRIPT = {"rules": [
    {"when": ["/echo:process-call"], "vars": {"dir": r"<command-args>(\S+)"},
     "steps": [{"tool": "Read", "input": {"file_path": "{dir}/transcript.md"}}, {"text": "done in {dir}"}]},
    {"when": "fix this diagram", "steps": [{"text": "flowchart LR\n  A --> B"}]},
]}


def user(text):
    return {"role": "user", "content": [{"type": "text", "text": text}]}


def run(folder):
    """A minutes run's first message as Claude Code sends it: the slash command, its arguments."""
    return user(f"<command-name>/echo:process-call</command-name>\n<command-args>{folder} more</command-args>")


class Answers(unittest.TestCase):
    def test_each_model_turn_takes_the_next_step_and_the_last_repeats(self):
        first = {"messages": [run("/data/calls/a")]}
        self.assertEqual(ms.answer_for(first, SCRIPT), (0, 0, {"tool": "Read", "input": {"file_path": "/data/calls/a/transcript.md"}}))
        after = {"messages": first["messages"] + [{"role": "assistant", "content": [{"type": "tool_use", "id": "t", "name": "Read", "input": {}}]},
                                                  {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "words"}]}]}
        self.assertEqual(ms.answer_for(after, SCRIPT)[1:], (1, {"text": "done in /data/calls/a"}))
        later = {"messages": after["messages"] + [{"role": "assistant", "content": "x"}, user("again")]}
        self.assertEqual(ms.answer_for(later, SCRIPT)[1:], (2, {"text": "done in /data/calls/a"}), "past the last step: the last again")

    def test_a_rule_is_matched_in_the_system_prompt_too_and_none_gets_the_no_script_text(self):
        self.assertEqual(ms.answer_for({"system": [{"type": "text", "text": "Please fix this diagram"}], "messages": [user("x")]}, SCRIPT)[0], 1)
        self.assertEqual(ms.answer_for({"messages": [user("hello")]}, SCRIPT), (None, 0, ms.NO_SCRIPT))


    def test_a_rule_with_no_phrases_matches_nothing_and_a_bad_regex_is_a_script_error(self):
        self.assertEqual(ms.answer_for({"messages": [user("x")]}, {"rules": [{"when": [], "steps": [{"text": "y"}]}]})[0], None)
        with self.assertRaises(ms.ScriptError):
            ms.answer_for({"messages": [user("x")]}, {"rules": [{"when": "x", "vars": {"a": "(unclosed"}, "steps": [{"text": "y"}]}]})


class Http(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        with open(os.path.join(self.dir, "model-script.json"), "w") as f:
            json.dump(SCRIPT | {"model": {"context_length": 8192}}, f)
        self.srv = ms.Server(("127.0.0.1", 0), self.dir)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.srv.server_address[1]}"

    def tearDown(self):
        self.srv.shutdown()
        self.srv.server_close()

    def post(self, path, body):
        req = urllib.request.Request(self.url + path, json.dumps(body).encode(), {"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as r:
            return r.headers.get("Content-Type"), r.read().decode()

    def test_a_plain_request_gets_a_message_and_is_logged(self):
        ctype, body = self.post("/v1/messages", {"model": "virtual", "max_tokens": 50, "messages": [run("/c/x")]})
        msg = json.loads(body)
        self.assertEqual((msg["stop_reason"], msg["content"][0]["type"], msg["content"][0]["name"]), ("tool_use", "tool_use", "Read"))
        with open(os.path.join(self.dir, "model-log.jsonl")) as f:
            log = [json.loads(l) for l in f]
        self.assertEqual((len(log), log[0]["rule"], log[0]["step"], log[0]["stream"]), (1, 0, 0, False))

    def test_a_streamed_request_gets_the_events_claude_code_reads(self):
        ctype, body = self.post("/v1/messages", {"model": "virtual", "stream": True, "max_tokens": 50,
                                                 "tools": [{"name": "Read"}], "messages": [run("/c/x")]})
        self.assertTrue(ctype.startswith("text/event-stream"))
        events = [l[len("event: "):] for l in body.splitlines() if l.startswith("event: ")]
        self.assertEqual(events, ["message_start", "content_block_start", "content_block_delta", "content_block_stop", "message_delta", "message_stop"])
        delta = [json.loads(l[6:]) for l in body.splitlines() if l.startswith("data: ") and "input_json_delta" in l][0]
        self.assertEqual(json.loads(delta["delta"]["partial_json"]), {"file_path": "/c/x/transcript.md"})
        with open(os.path.join(self.dir, "model-log.jsonl")) as f:
            self.assertEqual(json.loads(f.readline())["tools"], ["Read"])

    def test_a_bad_script_is_a_400_at_once_not_a_dropped_connection(self):
        with open(os.path.join(self.dir, "model-script.json"), "w") as f:
            json.dump({"rules": [{"when": "x", "vars": {"a": "(unclosed"}, "steps": [{"text": "y"}]}]}, f)
        with self.assertRaises(urllib.error.HTTPError) as e:
            self.post("/v1/messages", {"model": "virtual", "messages": [user("x")]})
        self.assertEqual(e.exception.code, 400)
        self.assertIn("isn't a regex", json.loads(e.exception.read())["error"]["message"])
        e.exception.close()

    def test_ollama_reports_the_scripts_context(self):
        with urllib.request.urlopen(self.url + "/api/ps", timeout=5) as r:
            self.assertEqual(json.load(r)["models"][0]["context_length"], 8192)


if __name__ == "__main__":
    sys.exit(0 if unittest.main(exit=False, verbosity=1).result.wasSuccessful() else 1)
