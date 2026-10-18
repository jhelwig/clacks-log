# Equipment modeling research

Checked 2026-10-12 while defining reusable equipment configurations. These are source findings and proposals, not new accepted requirements.

## Documented logger behavior

- [LOG4OM user manual, Station Configuration, page 164](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU.pdf): the primary-source search index describes rig, power, and antenna configuration, with radio → antenna → band associations and radio/antenna selection in My Station. Its example radio label includes an Elecraft KX3/KXPA100AT combination, but the inspected section does not document a separate amplifier field. Evidence here is the indexed manual section, not an exhaustive application inspection.
- [Ham Radio Deluxe (HRD) My Station documentation](https://support.hamradiodeluxe.com/support/solutions/articles/51000052682): Equipment is a description of the logging station equipment, with separate Antennas and Power fields. No dedicated amplifier field is documented in this page. The page loosely calls the own-equipment Amateur Data Interchange Format (ADIF) field Rig; the ADIF standard distinguishes MY_RIG from RIG.
- [N1MM Configurer](https://n1mmwp.hamdocs.com/setup/the-configurer/) and [Interfacing](https://n1mmwp.hamdocs.com/setup/interfacing/): amplifier references concern push-to-talk (PTT) timing and hardware interfacing/band control. This does not establish amplifier identity as a stored per-radio contact (QSO) field. A dedicated amplifier logging field was not found in the inspected documentation; absence across all versions/features is not proven.
- [ADIF 3.1.6](https://www.adif.org/316/ADIF_316.htm): MY_RIG describes the logging station equipment, MY_ANTENNA describes its antenna, and TX_PWR records transmitting power. No dedicated amplifier field is defined. An amplifier can be included in the equipment description without a separate standard field.

## Planning implication

The assistant's separate amplifier-field suggestion was not grounded in common logger behavior. Revised proposal: named configurations with an equipment description (including radio and optional other equipment), antenna description, and optional notes; power stays separate under D70. Exact field design, individual equipment catalogs, and minimum viable product (MVP) scope remain unaccepted.

## Concurrent references to deleted configurations (D183)

MVP keeps profile edits referencing deleted equipment pending for explicit reassignment or removal of the optional default. References accepted before deletion must be included in D73’s affected-profile review. Existing copied operating values and historical contact snapshots remain intact. Post-MVP, additional resolution options should allow restoring the configuration or cloning it to a new copy. Exact identity, recoverable content/retention, and transactional mechanics remain open; no indefinite full history or automatic restoration is implied.
