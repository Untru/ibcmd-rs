//! Bilingual event names supported by the BOOT-004 EventSubscription codec.

const EVENTS: &[(&str, &str)] = &[
    ("BeforeDelete", "BeforeDelete_ПередУдалением"),
    ("BeforeWrite", "BeforeWrite_ПередЗаписью"),
    (
        "FillCheckProcessing",
        "FillCheckProcessing_ОбработкаПроверкиЗаполнения",
    ),
    ("Filling", "Filling_ОбработкаЗаполнения"),
    (
        "FormGetProcessing",
        "FormGetProcessing_ОбработкаПолученияФормы",
    ),
    (
        "OnReceiveDataFromMaster",
        "OnReceiveDataFromMaster_ПриПолученииДанныхОтГлавного",
    ),
    (
        "OnReceiveDataFromSlave",
        "OnReceiveDataFromSlave_ПриПолученииДанныхОтПодчиненного",
    ),
    (
        "OnSendDataToMaster",
        "OnSendDataToMaster_ПриОтправкеДанныхГлавному",
    ),
    (
        "OnSendDataToSlave",
        "OnSendDataToSlave_ПриОтправкеДанныхПодчиненному",
    ),
    (
        "OnSendNodeDataToSlave",
        "OnSendNodeDataToSlave_ПриОтправкеДанныхУзлаПодчиненному",
    ),
    ("OnSetNewNumber", "OnSetNewNumber_ПриУстановкеНовогоНомера"),
    ("OnWrite", "OnWrite_ПриЗаписи"),
    ("Posting", "Posting_ОбработкаПроведения"),
    (
        "PresentationFieldsGetProcessing",
        "PresentationFieldsGetProcessing_ОбработкаПолученияПолейПредставления",
    ),
    (
        "PresentationGetProcessing",
        "PresentationGetProcessing_ОбработкаПолученияПредставления",
    ),
];

/// Look up a BOOT-004 XML event's stored spelling.
pub fn native_event_name(event: &str) -> Option<&'static str> {
    EVENTS
        .iter()
        .find(|(xml, _)| *xml == event)
        .map(|(_, native)| *native)
}

/// The descriptor writer supports five additional events beyond BOOT-004.
pub fn descriptor_native_event_name(event: &str) -> Option<&'static str> {
    match event {
        "OnCopy" => Some("OnCopy_ПриКопировании"),
        "OnSetNewCode" => Some("OnSetNewCode_ПриУстановкеНовогоКода"),
        "UndoPosting" => Some("UndoPosting_ОбработкаУдаленияПроведения"),
        "ChoiceDataGetProcessing" => Some("ChoiceDataGetProcessing_ОбработкаПолученияДанныхВыбора"),
        // Документооборот 3.0 `EventSubscriptions/ЗадачаКомплексногоПроцессаПриВыполнении`.
        "OnExecute" => Some("OnExecute_ПриВыполнении"),
        _ => native_event_name(event),
    }
}

/// Resolve the complete stored spelling, including its Russian suffix.
pub fn event_from_native(value: &str) -> Option<&'static str> {
    EVENTS
        .iter()
        .find(|(_, native)| *native == value)
        .map(|(xml, _)| *xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_events_round_trip_by_exact_spelling() {
        for &(xml, stored) in EVENTS {
            assert_eq!(native_event_name(xml), Some(stored));
            assert_eq!(event_from_native(stored), Some(xml));
        }
        assert_eq!(event_from_native("BeforeWrite_ПриЗаписи"), None);
        assert_eq!(event_from_native("BeforeWrite"), None);
        assert_eq!(native_event_name("OnExecute"), None);
        assert_eq!(native_event_name(""), None);
    }
}
