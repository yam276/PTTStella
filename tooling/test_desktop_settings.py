"""Probe discovery must stay bounded and never select a foreign browser."""
import json
import unittest
from unittest.mock import Mock, call, patch

import desktop_input


class SettingsTargetTests(unittest.TestCase):
    def discover(self, payload, status=200):
        connection = Mock()
        response = connection.getresponse.return_value
        response.status = status
        response.read.return_value = payload
        with patch.object(desktop_input.http.client, 'HTTPConnection', return_value=connection) as factory:
            try:
                return desktop_input.debug_pages(8123)
            finally:
                factory.assert_called_once_with('127.0.0.1', 8123, timeout=1)
                connection.close.assert_called_once()

    def test_loopback_targets_only_and_non_pages_ignored(self):
        address = 'ws://127.0.0.1:8123/devtools/page/settings'
        self.assertEqual(self.discover(json.dumps([
            {'type': 'page', 'webSocketDebuggerUrl': address},
            {'type': 'service_worker'},
        ]).encode()), [address])

    def test_foreign_port_host_scheme_missing_and_unbounded_lists_rejected(self):
        for address in ['ws://127.0.0.1:8124/page/x', 'ws://example.org:8123/page/x',
                        'wss://127.0.0.1:8123/page/x', None]:
            with self.subTest(address=address), self.assertRaises(ValueError):
                self.discover(json.dumps([{'type': 'page', 'webSocketDebuggerUrl': address}]).encode())
        for payload in [b'{}', b'[null]', b'['+b'{},'*8+b'{}]', b' '* (1024*1024+1)]:
            with self.subTest(size=len(payload)), self.assertRaises(ValueError):
                self.discover(payload)
        with self.assertRaises(ValueError):
            self.discover(b'[]', status=500)

    def test_settings_reuses_the_main_target_without_discovery_or_new_connection(self):
        main = desktop_input.DevTools.__new__(desktop_input.DevTools)
        main.call = Mock()
        main.click = Mock()
        main.wait = Mock()
        main.evaluate = Mock(return_value=False)
        with patch.object(desktop_input, 'debug_pages') as discovery:
            self.assertIs(main.open_settings(), main)
            self.assertIs(main.open_settings(), main)
            discovery.assert_not_called()
        self.assertEqual(main.click.call_args_list, [
            call('#tools-ball'), call('[data-tools-target="settings"]'),
            call('#tools-ball'), call('[data-tools-target="settings"]'),
        ])
        self.assertEqual(main.call.call_args_list, [call('Page.bringToFront')] * 2)
        self.assertEqual(main.wait.call_count, 2)

    def test_settings_closes_an_open_tool_group_before_opening_root_settings(self):
        main = desktop_input.DevTools.__new__(desktop_input.DevTools)
        main.call = Mock()
        main.click = Mock()
        main.wait = Mock()
        main.evaluate = Mock(side_effect=[True, True])
        with patch.object(desktop_input, 'debug_pages') as discovery:
            self.assertIs(main.open_settings(), main)
            discovery.assert_not_called()
        # Back to the root, close the fan, then reopen at the settings entry.
        self.assertEqual(main.click.call_args_list, [call('#tools-ball')] * 3 + [
            call('[data-tools-target="settings"]'),
        ])
        main.wait.assert_called_once_with(
            'document.getElementById("font-dialog").open',
            'in-app settings did not become ready')


if __name__ == '__main__':
    unittest.main()
