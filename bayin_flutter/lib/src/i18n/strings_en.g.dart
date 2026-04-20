///
/// Generated file. Do not edit.
///
// coverage:ignore-file
// ignore_for_file: type=lint, unused_import
// dart format off

part of 'strings.g.dart';

// Path: <root>
typedef TranslationsEn = Translations; // ignore: unused_element
class Translations with BaseTranslations<AppLocale, Translations> {
	/// Returns the current translations of the given [context].
	///
	/// Usage:
	/// final t = Translations.of(context);
	static Translations of(BuildContext context) => InheritedLocaleData.of<AppLocale, Translations>(context).translations;

	/// You can call this constructor and build your own translation instance of this locale.
	/// Constructing via the enum [AppLocale.build] is preferred.
	Translations({Map<String, Node>? overrides, PluralResolver? cardinalResolver, PluralResolver? ordinalResolver, TranslationMetadata<AppLocale, Translations>? meta})
		: assert(overrides == null, 'Set "translation_overrides: true" in order to enable this feature.'),
		  $meta = meta ?? TranslationMetadata(
		    locale: AppLocale.en,
		    overrides: overrides ?? {},
		    cardinalResolver: cardinalResolver,
		    ordinalResolver: ordinalResolver,
		  );

	/// Metadata for the translations of <en>.
	@override final TranslationMetadata<AppLocale, Translations> $meta;

	late final Translations _root = this; // ignore: unused_field

	Translations $copyWith({TranslationMetadata<AppLocale, Translations>? meta}) => Translations(meta: meta ?? this.$meta);

	// Translations
	late final TranslationsNavEn nav = TranslationsNavEn.internal(_root);
	late final TranslationsCommonEn common = TranslationsCommonEn.internal(_root);
}

// Path: nav
class TranslationsNavEn {
	TranslationsNavEn.internal(this._root);

	final Translations _root; // ignore: unused_field

	// Translations

	/// en: 'Library'
	String get library => 'Library';

	/// en: 'Songs'
	String get songs => 'Songs';

	/// en: 'Albums'
	String get albums => 'Albums';

	/// en: 'Artists'
	String get artists => 'Artists';

	/// en: 'Playlists'
	String get playlists => 'Playlists';

	/// en: 'System'
	String get system => 'System';

	/// en: 'Scan Music'
	String get scanMusic => 'Scan Music';

	/// en: 'Library Stats'
	String get libraryStats => 'Library Stats';

	/// en: 'Settings'
	String get settings => 'Settings';

	/// en: 'About'
	String get about => 'About';
}

// Path: common
class TranslationsCommonEn {
	TranslationsCommonEn.internal(this._root);

	final Translations _root; // ignore: unused_field

	// Translations

	/// en: 'BaYin'
	String get appName => 'BaYin';

	/// en: 'Back'
	String get back => 'Back';

	/// en: 'Close'
	String get close => 'Close';

	/// en: 'Copy'
	String get copy => 'Copy';

	/// en: 'Copied'
	String get copied => 'Copied';

	/// en: 'Details'
	String get details => 'Details';

	/// en: 'Exit'
	String get exit => 'Exit';

	/// en: 'Open sidebar'
	String get openSidebar => 'Open sidebar';

	/// en: 'Search'
	String get search => 'Search';

	/// en: 'Sort'
	String get sort => 'Sort';

	/// en: 'Refresh'
	String get refresh => 'Refresh';

	/// en: 'More'
	String get more => 'More';

	/// en: 'Toggle dark mode'
	String get toggleDarkMode => 'Toggle dark mode';

	/// en: 'Minimize'
	String get minimize => 'Minimize';

	/// en: 'Maximize'
	String get maximize => 'Maximize';

	/// en: 'Restore'
	String get restore => 'Restore';

	/// en: 'Close window'
	String get closeWindow => 'Close window';

	/// en: 'OK'
	String get ok => 'OK';

	/// en: 'Cancel'
	String get cancel => 'Cancel';

	/// en: 'Save'
	String get save => 'Save';

	/// en: 'Delete'
	String get delete => 'Delete';

	/// en: 'Edit'
	String get edit => 'Edit';

	/// en: 'Loading...'
	String get loading => 'Loading...';

	/// en: 'Error'
	String get error => 'Error';

	/// en: 'Success'
	String get success => 'Success';

	/// en: 'Confirm'
	String get confirm => 'Confirm';

	/// en: 'Play'
	String get play => 'Play';

	/// en: 'Shuffle'
	String get shuffle => 'Shuffle';

	/// en: 'Select'
	String get select => 'Select';
}
