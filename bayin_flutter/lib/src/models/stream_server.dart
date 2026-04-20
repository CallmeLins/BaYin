import '../rust/rust_api.dart';

class StreamServer {
  const StreamServer({
    required this.id,
    required this.serverType,
    required this.serverName,
    required this.serverUrl,
    required this.username,
    required this.enabled,
    required this.createdAt,
    this.accessToken,
    this.userId,
  });

  factory StreamServer.fromRust(RustDbStreamServer value) {
    return StreamServer(
      id: value.id,
      serverType: value.serverType,
      serverName: value.serverName,
      serverUrl: value.serverUrl,
      username: value.username,
      accessToken: value.accessToken,
      userId: value.userId,
      enabled: value.enabled,
      createdAt: value.createdAt,
    );
  }

  /// Intentionally omits the persisted password from this surface — callers
  /// that need the credential go through the Rust side (streaming clients)
  /// rather than reading the Dart model.
  final String id;
  final String serverType;
  final String serverName;
  final String serverUrl;
  final String username;
  final String? accessToken;
  final String? userId;
  final bool enabled;
  final int createdAt;
}
