# Rails 애플리케이션 에러 해결 가이드

## 발견된 문제점들

1. **Holiday 라우트 미정의**: `/holidays/:year/:month` 와 `/api/holidays/:year/:month` 경로가 정의되지 않음
   - 2025년 7월처럼 공휴일이 없는 달도 정상적으로 처리해야 함
2. **Task 모델 status 메서드 미정의**: Task 모델에 status 메서드가 없음
3. **잘못된 API 경로 접근**: 클라이언트가 존재하지 않는 `/api/v1/tasks` 경로에 접근 (v1은 사용하지 않음)
4. **Task 생성 시 에러**: Task 컨트롤러의 create 액션에서 에러 발생

## 해결 방법

### 1. 마이그레이션 실행
```bash
rails db:migrate
```

### 2. 서버 재시작
```bash
rails restart
# 또는
systemctl restart puma
# 또는
bundle exec puma -C config/puma.rb
```

### 3. 라우트 확인
```bash
rails routes | grep holiday
rails routes | grep task
```

### 4. 콘솔에서 Task 모델 확인
```bash
rails console
Task.new.status  # 이제 에러가 발생하지 않아야 함
```

## 추가 권장사항

1. **모니터링 도구 설정**
   - Sentry 또는 Rollbar 같은 에러 모니터링 도구 설정
   - 실시간으로 에러를 추적하고 알림 받기

2. **테스트 추가**
   ```ruby
   # spec/models/task_spec.rb
   RSpec.describe Task, type: :model do
     describe '#status' do
       it 'returns active for incomplete tasks' do
         task = Task.new
         expect(task.status).to eq('active')
       end
       
       it 'returns completed for completed tasks' do
         task = Task.new(completed_at: Time.current)
         expect(task.status).to eq('completed')
       end
       
       it 'returns deleted for deleted tasks' do
         task = Task.new(deleted_at: Time.current)
         expect(task.status).to eq('deleted')
       end
     end
   end
   ```

3. **API 문서화**
   - Swagger 또는 API Blueprint를 사용하여 API 문서화
   - 클라이언트 개발자에게 정확한 엔드포인트 정보 제공

4. **로그 레벨 조정**
   ```ruby
   # config/environments/production.rb
   config.log_level = :info  # 너무 많은 로그 방지
   ```

5. **헬스체크 엔드포인트 추가**
   ```ruby
   # config/routes.rb
   get '/health', to: proc { [200, {}, ['OK']] }
   ```