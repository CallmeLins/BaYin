///
/// Generated file. Do not edit.
///
// coverage:ignore-file
// ignore_for_file: type=lint, unused_import
// dart format off

import 'package:flutter/widgets.dart';
import 'package:intl/intl.dart';
import 'package:slang/generated.dart';
import 'strings.g.dart';

// Path: <root>
class TranslationsZhCn extends Translations with BaseTranslations<AppLocale, Translations> {
	/// You can call this constructor and build your own translation instance of this locale.
	/// Constructing via the enum [AppLocale.build] is preferred.
	TranslationsZhCn({Map<String, Node>? overrides, PluralResolver? cardinalResolver, PluralResolver? ordinalResolver, TranslationMetadata<AppLocale, Translations>? meta})
		: assert(overrides == null, 'Set "translation_overrides: true" in order to enable this feature.'),
		  $meta = meta ?? TranslationMetadata(
		    locale: AppLocale.zhCn,
		    overrides: overrides ?? {},
		    cardinalResolver: cardinalResolver,
		    ordinalResolver: ordinalResolver,
		  ),
		  super(cardinalResolver: cardinalResolver, ordinalResolver: ordinalResolver);

	/// Metadata for the translations of <zh-CN>.
	@override final TranslationMetadata<AppLocale, Translations> $meta;

	late final TranslationsZhCn _root = this; // ignore: unused_field

	@override 
	TranslationsZhCn $copyWith({TranslationMetadata<AppLocale, Translations>? meta}) => TranslationsZhCn(meta: meta ?? this.$meta);

	// Translations
	@override late final TranslationsNavZhCn nav = TranslationsNavZhCn._(_root);
	@override late final TranslationsCommonZhCn common = TranslationsCommonZhCn._(_root);
}

// Path: nav
class TranslationsNavZhCn extends TranslationsNavEn {
	TranslationsNavZhCn._(TranslationsZhCn root) : this._root = root, super.internal(root);

	final TranslationsZhCn _root; // ignore: unused_field

	// Translations
	@override String get library => '音乐库';
	@override String get songs => '歌曲';
	@override String get albums => '专辑';
	@override String get artists => '艺术家';
	@override String get playlists => '歌单';
	@override String get system => '系统';
	@override String get scanMusic => '扫描音乐';
	@override String get libraryStats => '音乐库统计';
	@override String get settings => '设置';
	@override String get about => '关于';
}

// Path: common
class TranslationsCommonZhCn extends TranslationsCommonEn {
	TranslationsCommonZhCn._(TranslationsZhCn root) : this._root = root, super.internal(root);

	final TranslationsZhCn _root; // ignore: unused_field

	// Translations
	@override String get appName => '八音';
	@override String get back => '返回';
	@override String get close => '关闭';
	@override String get copy => '复制';
	@override String get copied => '已复制';
	@override String get details => '详情';
	@override String get exit => '退出';
	@override String get openSidebar => '打开侧边栏';
	@override String get search => '搜索';
	@override String get sort => '排序';
	@override String get refresh => '刷新';
	@override String get more => '更多';
	@override String get toggleDarkMode => '切换深色模式';
	@override String get minimize => '最小化';
	@override String get maximize => '最大化';
	@override String get restore => '还原';
	@override String get closeWindow => '关闭窗口';
	@override String get ok => '确定';
	@override String get cancel => '取消';
	@override String get save => '保存';
	@override String get delete => '删除';
	@override String get edit => '编辑';
	@override String get loading => '加载中...';
	@override String get error => '错误';
	@override String get success => '成功';
	@override String get confirm => '确定';
	@override String get play => '播放';
	@override String get shuffle => '随机播放';
	@override String get select => '选择';
}
