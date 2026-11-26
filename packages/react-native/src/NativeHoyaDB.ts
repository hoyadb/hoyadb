// API-0011 owned-byte native capabilities; no database semantics in this module.
import type {TurboModule, CodegenTypes} from 'react-native';
import {TurboModuleRegistry} from 'react-native';

export interface Spec extends TurboModule {
  open(input: string, key: ReadonlyArray<number>): Promise<string>;
  call(input: string): Promise<string>;
  namespace(): Promise<ReadonlyArray<number>>;
  uptime(): string;
  readonly onAdmissionReleased: CodegenTypes.EventEmitter<void>;
}

export default TurboModuleRegistry.getEnforcing<Spec>('HoyaDB');
