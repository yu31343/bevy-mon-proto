import sys
try:
    with open('c:/Users/31343/AppData/Roaming/Code/User/workspaceStorage/6399b3927c22fdaf2b21fd2b36882ad7/GitHub.copilot-chat/chat-session-resources/519d74e7-3f09-4e1a-a486-cc6ba3e64b06/call_MHx1RU5aNTkwU0lWY2RMY0p1OFI__vscode-1774881754048/content.txt', 'r', encoding='utf-8') as f:
        c = f.read()
    parts = c.split('```rust\n')
    if len(parts) > 1:
        m = parts[1].split('\n```')[0]
        with open('d:/31343/Documents/vscodeFiles/rustfiles/github_projects/bevy-mon-proto/src/ui/mod.rs', 'w', encoding='utf-8') as f:
            f.write(m)
        print('Success')
    else:
        print('No marker found')
except Exception as e:
    print('Error:', e)
