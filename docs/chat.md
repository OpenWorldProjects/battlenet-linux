# Experimental Battle.net chat

Open **Chats and friends** and press **Connect** after signing into the launcher. The connection uses the selected game region (EU, US, or KR). Blizzard may open the managed sign-in window to authorize its `app` client. This is separate from a game ticket; it uses the same private browser session.

The Rust backend implements TLS-verified Battle.net v2 framed protobuf RPC, current bindless service hashes, server web authorization challenges, account identity matching, friend subscriptions, BattleTag presence queries, incoming whispers, and outgoing whispers with server acknowledgement. Contacts and messages appear only from server responses. No online presence is fabricated. Successful send acknowledgement means the server accepted the request, not that a recipient read it.

Incoming messages request desktop attention and a generic desktop notification without including their contents. The connection continues while the chat window is closed. Chat history and the chat ticket are memory-only. Disconnect/sign-out clears the local conversation state. Group chat, invitations, historical message synchronization, rich presence, and persistent chat credentials are not implemented.

The public EU service's TLS certificate, connection handshake, bindless RPC mode, anonymous App logon request, and returned web-auth challenge were checked without account credentials. Account authorization and friend-list loading were subsequently observed working in the local client. The contact-name query was corrected to use the current entity/key field numbers; live name resolution and actual private-conversation delivery still need confirmation. No test messages were sent. Errors remain visible in the chat window; an unavailable backend does not simulate a successful connection.

Protocol field references (independent Rust implementation; no foreign implementation source is bundled):

- https://github.com/EionRobb/purple-battlenet — legacy service hashes, method numbers, and whisper format.
- https://github.com/superp00t/gophercraft/tree/master/bnet/bgs/protocol — modern Header, connection, authentication, friends, presence, and notification field definitions.

The server-supplied challenge URL is passed to the sign-in helper through stdin, not command-line arguments. The helper accepts only HTTPS Blizzard login hosts, replaces the callback with its managed return URI, and validates the returned ticket against the signed-in account. Chat tickets, callback URLs, message contents, and contact names are not written to diagnostic logs. Certificate verification is never disabled.
