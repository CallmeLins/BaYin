import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../services/library_service.dart';
import '../../widgets/widgets.dart';

class StreamServerConfigPage extends ConsumerStatefulWidget {
  const StreamServerConfigPage({super.key});

  @override
  ConsumerState<StreamServerConfigPage> createState() =>
      _StreamServerConfigPageState();
}

class _StreamServerConfigPageState extends ConsumerState<StreamServerConfigPage> {
  static const List<_ServerTypeOption> _serverTypes = <_ServerTypeOption>[
    _ServerTypeOption('navidrome', 'Navidrome'),
    _ServerTypeOption('subsonic', 'Subsonic'),
    _ServerTypeOption('opensubsonic', 'OpenSubsonic'),
    _ServerTypeOption('jellyfin', 'Jellyfin'),
    _ServerTypeOption('emby', 'Emby'),
  ];

  final TextEditingController _serverNameController = TextEditingController();
  final TextEditingController _serverUrlController = TextEditingController();
  final TextEditingController _usernameController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();

  String _serverType = 'navidrome';
  String? _selectedServerId;
  String? _accessToken;
  String? _userId;

  bool _isTesting = false;
  bool _isSaving = false;
  bool _isSyncing = false;
  bool _isDeleting = false;

  @override
  void dispose() {
    _serverNameController.dispose();
    _serverUrlController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final asyncServers = ref.watch(streamServersProvider);
    return Column(
      children: [
        _Header(
          onOpenPlaylists: () => context.go('/playlists'),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              asyncServers.when(
                loading: () => const LinearProgressIndicator(minHeight: 2),
                error: (error, _) => _ErrorCard(message: '$error'),
                data: (servers) => _ServerListCard(
                  servers: servers,
                  selectedServerId: _selectedServerId,
                  onSelect: _handleSelectServer,
                  onCreateNew: _resetForm,
                ),
              ),
              const SizedBox(height: 12),
              _FormCard(
                serverType: _serverType,
                serverTypes: _serverTypes,
                serverNameController: _serverNameController,
                serverUrlController: _serverUrlController,
                usernameController: _usernameController,
                passwordController: _passwordController,
                onServerTypeChanged: (value) => setState(() {
                  _serverType = value;
                  _accessToken = null;
                  _userId = null;
                }),
              ),
              const SizedBox(height: 12),
              _ActionButtons(
                hasSelectedServer: _selectedServerId != null,
                isTesting: _isTesting,
                isSaving: _isSaving,
                isSyncing: _isSyncing,
                isDeleting: _isDeleting,
                onTest: _handleTestConnection,
                onSave: _handleSave,
                onSync: _handleSyncPlaylists,
                onDelete: _handleDelete,
              ),
            ],
          ),
        ),
      ],
    );
  }

  void _handleSelectServer(StreamServer server) {
    setState(() {
      _selectedServerId = server.id;
      _serverType = server.serverType;
      _serverNameController.text = server.serverName;
      _serverUrlController.text = server.serverUrl;
      _usernameController.text = server.username;
      _passwordController.clear();
      _accessToken = server.accessToken;
      _userId = server.userId;
    });
  }

  void _resetForm() {
    setState(() {
      _selectedServerId = null;
      _serverType = 'navidrome';
      _serverNameController.clear();
      _serverUrlController.clear();
      _usernameController.clear();
      _passwordController.clear();
      _accessToken = null;
      _userId = null;
    });
  }

  bool _validateForm() {
    final serverUrl = _serverUrlController.text.trim();
    final username = _usernameController.text.trim();
    final password = _passwordController.text;
    if (serverUrl.isEmpty || username.isEmpty || password.isEmpty) {
      _showMessage('Server URL, username and password are required.');
      return false;
    }
    return true;
  }

  Future<void> _handleTestConnection() async {
    if (!_validateForm() || _isTesting) return;
    setState(() => _isTesting = true);
    try {
      final result = await LibraryService.instance.testStreamConnection(
        serverType: _serverType,
        serverName: _normalizedServerName(),
        serverUrl: _serverUrlController.text.trim(),
        username: _usernameController.text.trim(),
        password: _passwordController.text,
        accessToken: _accessToken,
        userId: _userId,
      );
      if (!mounted) return;
      setState(() {
        if (result.accessToken != null && result.accessToken!.isNotEmpty) {
          _accessToken = result.accessToken;
        }
        if (result.userId != null && result.userId!.isNotEmpty) {
          _userId = result.userId;
        }
      });
      final suffix = result.serverVersion == null
          ? ''
          : ' (v${result.serverVersion})';
      _showMessage(
        '${result.message}$suffix',
        isError: !result.success,
      );
    } catch (error) {
      _showMessage('Connection test failed: $error', isError: true);
    } finally {
      if (mounted) {
        setState(() => _isTesting = false);
      }
    }
  }

  Future<void> _handleSave() async {
    if (!_validateForm() || _isSaving) return;
    setState(() => _isSaving = true);
    try {
      final serverId = await LibraryService.instance.saveStreamServer(
        serverType: _serverType,
        serverName: _normalizedServerName(),
        serverUrl: _serverUrlController.text.trim(),
        username: _usernameController.text.trim(),
        password: _passwordController.text,
        accessToken: _accessToken,
        userId: _userId,
      );
      if (!mounted) return;
      setState(() => _selectedServerId = serverId);
      ref.invalidate(streamServersProvider);
      ref.invalidate(streamPlaylistsProvider(serverId));
      _showMessage('Server saved.');
    } catch (error) {
      _showMessage('Failed to save server: $error', isError: true);
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _handleSyncPlaylists() async {
    final serverId = _selectedServerId;
    if (serverId == null || _isSyncing) {
      _showMessage('Select or save a server first.');
      return;
    }
    setState(() => _isSyncing = true);
    try {
      final result = await LibraryService.instance.syncStreamPlaylists(serverId);
      if (!mounted) return;
      ref.invalidate(streamPlaylistsProvider(serverId));
      _showMessage(
        'Synced ${result.playlistCount} playlists from ${result.serverName}.',
      );
    } catch (error) {
      _showMessage('Failed to sync playlists: $error', isError: true);
    } finally {
      if (mounted) {
        setState(() => _isSyncing = false);
      }
    }
  }

  Future<void> _handleDelete() async {
    final serverId = _selectedServerId;
    if (serverId == null || _isDeleting) {
      _showMessage('Select a server to delete.');
      return;
    }
    setState(() => _isDeleting = true);
    try {
      await LibraryService.instance.deleteStreamServer(serverId);
      if (!mounted) return;
      ref.invalidate(streamServersProvider);
      ref.invalidate(streamPlaylistsProvider(serverId));
      _resetForm();
      _showMessage('Server deleted.');
    } catch (error) {
      _showMessage('Failed to delete server: $error', isError: true);
    } finally {
      if (mounted) {
        setState(() => _isDeleting = false);
      }
    }
  }

  String _normalizedServerName() {
    final raw = _serverNameController.text.trim();
    if (raw.isNotEmpty) {
      return raw;
    }
    for (final item in _serverTypes) {
      if (item.value == _serverType) {
        return item.label;
      }
    }
    return _serverType;
  }

  void _showMessage(String message, {bool isError = false}) {
    if (!mounted) return;
    final messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: isError ? Theme.of(context).colorScheme.error : null,
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.onOpenPlaylists});

  final VoidCallback onOpenPlaylists;

  @override
  Widget build(BuildContext context) {
    return BayinPageHeader(
      title: const Text('Stream Servers'),
      left: IconButton(
        tooltip: 'Back',
        onPressed: () {
          if (context.canPop()) {
            context.pop();
          } else {
            context.go('/playlists');
          }
        },
        icon: Icon(PhosphorIcons.caretLeft()),
      ),
      right: TextButton.icon(
        onPressed: onOpenPlaylists,
        icon: Icon(PhosphorIcons.playlist()),
        label: const Text('Playlists'),
      ),
    );
  }
}

class _ServerListCard extends StatelessWidget {
  const _ServerListCard({
    required this.servers,
    required this.selectedServerId,
    required this.onSelect,
    required this.onCreateNew,
  });

  final List<StreamServer> servers;
  final String? selectedServerId;
  final ValueChanged<StreamServer> onSelect;
  final VoidCallback onCreateNew;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return BayinGlassCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 8),
            child: Row(
              children: [
                const Text(
                  'Saved servers',
                  style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
                ),
                const Spacer(),
                TextButton(
                  onPressed: onCreateNew,
                  child: const Text('New'),
                ),
              ],
            ),
          ),
          if (servers.isEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
              child: Text(
                'No stream servers configured yet.',
                style: TextStyle(
                  fontSize: 12,
                  color: scheme.onSurfaceVariant,
                ),
              ),
            )
          else
            for (final server in servers)
              Material(
                color: Colors.transparent,
                child: InkWell(
                  onTap: () => onSelect(server),
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
                    child: Row(
                      children: [
                        Icon(PhosphorIcons.cloud(), size: 16),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            '${server.serverName} (${server.serverType})',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontSize: 13),
                          ),
                        ),
                        if (server.id == selectedServerId)
                          Icon(
                            PhosphorIcons.checkCircle(PhosphorIconsStyle.fill),
                            size: 16,
                            color: Theme.of(context).colorScheme.primary,
                          ),
                      ],
                    ),
                  ),
                ),
              ),
          const SizedBox(height: 4),
        ],
      ),
    );
  }
}

class _FormCard extends StatelessWidget {
  const _FormCard({
    required this.serverType,
    required this.serverTypes,
    required this.serverNameController,
    required this.serverUrlController,
    required this.usernameController,
    required this.passwordController,
    required this.onServerTypeChanged,
  });

  final String serverType;
  final List<_ServerTypeOption> serverTypes;
  final TextEditingController serverNameController;
  final TextEditingController serverUrlController;
  final TextEditingController usernameController;
  final TextEditingController passwordController;
  final ValueChanged<String> onServerTypeChanged;

  @override
  Widget build(BuildContext context) {
    final decoration = const InputDecoration(
      border: OutlineInputBorder(),
      isDense: true,
    );
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          DropdownButtonFormField<String>(
            initialValue: serverType,
            decoration: decoration.copyWith(labelText: 'Server type'),
            items: serverTypes
                .map(
                  (item) => DropdownMenuItem<String>(
                    value: item.value,
                    child: Text(item.label),
                  ),
                )
                .toList(growable: false),
            onChanged: (value) {
              if (value == null) return;
              onServerTypeChanged(value);
            },
          ),
          const SizedBox(height: 10),
          TextField(
            controller: serverNameController,
            decoration: decoration.copyWith(labelText: 'Server name (optional)'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: serverUrlController,
            decoration: decoration.copyWith(labelText: 'Server URL'),
            keyboardType: TextInputType.url,
          ),
          const SizedBox(height: 10),
          TextField(
            controller: usernameController,
            decoration: decoration.copyWith(labelText: 'Username'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: passwordController,
            decoration: decoration.copyWith(labelText: 'Password'),
            obscureText: true,
          ),
        ],
      ),
    );
  }
}

class _ActionButtons extends StatelessWidget {
  const _ActionButtons({
    required this.hasSelectedServer,
    required this.isTesting,
    required this.isSaving,
    required this.isSyncing,
    required this.isDeleting,
    required this.onTest,
    required this.onSave,
    required this.onSync,
    required this.onDelete,
  });

  final bool hasSelectedServer;
  final bool isTesting;
  final bool isSaving;
  final bool isSyncing;
  final bool isDeleting;
  final VoidCallback onTest;
  final VoidCallback onSave;
  final VoidCallback onSync;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Row(
          children: [
            Expanded(
              child: OutlinedButton.icon(
                onPressed: isTesting ? null : onTest,
                icon: isTesting
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(PhosphorIcons.plugs()),
                label: const Text('Test'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: FilledButton.icon(
                onPressed: isSaving ? null : onSave,
                icon: isSaving
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: Colors.white,
                        ),
                      )
                    : Icon(PhosphorIcons.floppyDisk()),
                label: const Text('Save'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            Expanded(
              child: OutlinedButton.icon(
                onPressed: hasSelectedServer && !isSyncing ? onSync : null,
                icon: isSyncing
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(PhosphorIcons.arrowsClockwise()),
                label: const Text('Sync playlists'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: FilledButton.tonalIcon(
                onPressed: hasSelectedServer && !isDeleting ? onDelete : null,
                icon: isDeleting
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(PhosphorIcons.trash()),
                label: const Text('Delete'),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _ErrorCard extends StatelessWidget {
  const _ErrorCard({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.errorContainer,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Text(
        message,
        style: TextStyle(
          color: Theme.of(context).colorScheme.onErrorContainer,
          fontSize: 12,
        ),
      ),
    );
  }
}

class _ServerTypeOption {
  const _ServerTypeOption(this.value, this.label);

  final String value;
  final String label;
}
