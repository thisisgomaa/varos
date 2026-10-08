# Picker risk register

- Open: the picker has no scrim. A canvas press or Bridge edit mid-session can run `begin()` and overwrite its undo snapshot; Cancel may then restore the wrong document and OK may lose the original undo boundary. K3 rule 5 (refuse a foreign begin while a transaction owns the snapshot) is the proper fix, tracked as a separate piece. This fix round does not implement that transaction ownership rule.
