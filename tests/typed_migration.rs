use dekopon_provider_sdk::provider::{Capabilities, Capability, ImportSet, Needs, Provider};
use dekopon_ripgrep_provider::{RipgrepProvider, Search};

#[test]
fn typed_sdk_declares_a_single_import_free_search() {
    fn typed<P: Provider>() {}
    typed::<RipgrepProvider>();
    assert_eq!(<RipgrepProvider as Provider>::ID, "ripgrep");
    assert_eq!(<Search as Capability>::NAME, "search");
    assert_eq!(
        <<Search as Capability>::Needs as Needs>::IMPORTS,
        ImportSet::EMPTY
    );
    assert_eq!(
        <<RipgrepProvider as Provider>::Capabilities as Capabilities<RipgrepProvider>>::IMPORTS,
        ImportSet::EMPTY
    );
}
