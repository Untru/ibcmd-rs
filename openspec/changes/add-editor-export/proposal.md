# Editor selective export (#381)

Add editor commands for selected objects, a metadata group and native changed objects. Prepare exact source bytes and inventory in a separate retained staging folder before asking to overwrite local changes. Refs #381; native query semantics depend on #485, connection/status UI on #477/#484. This branch merges those donor commits without changing their branches.
