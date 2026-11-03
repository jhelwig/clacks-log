SELECT (SELECT count(*) FROM record_identity) AS records,
       (SELECT count(*) FROM installed_checkpoint) AS checkpoints
