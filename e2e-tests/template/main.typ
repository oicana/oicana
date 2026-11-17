#import "@local/oicana:0.2.1": setup
#import "sub/module.typ": sub-module

#let read-project-file(path) = return read(path, encoding: none);
#let (input, _, _) = setup(read-project-file);

#set document(date: datetime.today())

Contains #input.len() inputs (should be 6)\

`default-blob` has value: #str(input.default-blob.bytes)\
`default-blob` has meta: #repr(input.default-blob.meta)\

`development-blob` has value: #str(input.development-blob.bytes)\
`development-blob` has meta: #repr(input.development-blob.meta)\

`both-blob` has value: #str(input.both-blob.bytes)\
`both-blob` has meta: #repr(input.both-blob.meta)\

`default-json` has value: #input.default-json\
`development-json` has value: #input.development-json\
`both-json` has value: #input.both-json\

#sub-module
